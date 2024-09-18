use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    message::{CreateMessage, Message, UpdateMessage},
    thread::Thread,
};

#[derive(Clone)]
pub struct Database {
    threads: Arc<Mutex<HashMap<Uuid, Thread>>>,
    messages: Arc<Mutex<HashMap<Uuid, Message>>>,
    thread_messages: Arc<Mutex<HashMap<Uuid, HashSet<Uuid>>>>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            threads: Arc::new(Mutex::new(HashMap::new())),
            messages: Arc::new(Mutex::new(HashMap::new())),
            thread_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_thread(&self) -> Thread {
        let thread = Thread::new();
        let mut threads = self.threads.lock().await;
        threads.insert(thread.id(), thread.clone());
        self.thread_messages
            .lock()
            .await
            .insert(thread.id(), HashSet::new());
        thread
    }

    pub async fn delete_thread(&self, thread_id: Uuid) -> Result<()> {
        let mut threads = self.threads.lock().await;
        if threads.remove(&thread_id).is_none() {
            return Err(anyhow!("thread not found"));
        }

        let mut messages = self.messages.lock().await;
        let mut thread_messages = self.thread_messages.lock().await;

        if let Some(message_ids) = thread_messages.remove(&thread_id) {
            for message_id in message_ids {
                messages.remove(&message_id);
            }
        }

        Ok(())
    }

    pub async fn create_message(&self, thread_id: Uuid, input: CreateMessage) -> Result<Message> {
        self.threads
            .lock()
            .await
            .get(&thread_id)
            .ok_or_else(|| anyhow!("thread not found"))?;

        let message = input.into_message(thread_id);
        let message_id = message.id();
        let mut messages = self.messages.lock().await;
        messages.insert(message_id, message.clone());

        let mut thread_messages = self.thread_messages.lock().await;
        thread_messages
            .entry(thread_id)
            .or_insert_with(HashSet::new)
            .insert(message_id);

        Ok(message)
    }

    pub async fn update_message(
        &self,
        thread_id: Uuid,
        message_id: Uuid,
        content: UpdateMessage,
    ) -> Result<Message> {
        self.threads
            .lock()
            .await
            .get(&thread_id)
            .ok_or_else(|| anyhow!("thread not found"))?;

        let mut messages = self.messages.lock().await;
        let message = messages
            .get_mut(&message_id)
            .ok_or_else(|| anyhow!("message not found"))?;

        message.update_content(content);
        Ok(message.clone())
    }

    pub async fn list_threads(&self) -> Vec<Thread> {
        let threads = self.threads.lock().await;
        threads.values().cloned().collect()
    }

    pub async fn get_thread(&self, thread_id: Uuid) -> Result<Thread> {
        let threads = self.threads.lock().await;
        threads
            .get(&thread_id)
            .cloned()
            .ok_or_else(|| anyhow!("thread not found"))
    }

    pub async fn get_thread_messages(&self, thread_id: Uuid) -> Result<Vec<Message>> {
        let threads = self.threads.lock().await;
        if !threads.contains_key(&thread_id) {
            return Err(anyhow!("thread not found"));
        }

        let thread_messages = self.thread_messages.lock().await;
        let messages = self.messages.lock().await;

        let message_ids = thread_messages.get(&thread_id).cloned().unwrap_or_default();
        let mut thread_messages: Vec<Message> = message_ids
            .iter()
            .filter_map(|id| messages.get(id).cloned())
            .collect();

        thread_messages.sort_by(|a, b| a.created_at().cmp(&b.created_at()));

        Ok(thread_messages)
    }

    pub async fn delete_message(&self, thread_id: Uuid, message_id: Uuid) -> Result<()> {
        self.threads
            .lock()
            .await
            .get(&thread_id)
            .ok_or_else(|| anyhow!("thread not found"))?;

        let mut messages = self.messages.lock().await;
        messages
            .remove(&message_id)
            .ok_or_else(|| anyhow!("message not found"))?;

        Ok(())
    }
}
