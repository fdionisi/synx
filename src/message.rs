use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentKind {
    Text { text: String },
    Image { url: String },
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum Content {
    Single(ContentKind),
    Multiple(Vec<ContentKind>),
}

impl From<String> for Content {
    fn from(text: String) -> Self {
        Content::Single(ContentKind::Text { text })
    }
}

impl From<Vec<String>> for Content {
    fn from(texts: Vec<String>) -> Self {
        Content::Multiple(
            texts
                .into_iter()
                .map(|text| ContentKind::Text { text })
                .collect(),
        )
    }
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ContentHelper {
            Single(String),
            Multiple(Vec<String>),
            SingleObject(ContentKind),
            MultipleObjects(Vec<ContentKind>),
        }

        let helper = ContentHelper::deserialize(deserializer)?;
        match helper {
            ContentHelper::Single(text) => Ok(Content::Single(ContentKind::Text { text })),
            ContentHelper::Multiple(texts) => Ok(Content::Multiple(
                texts
                    .into_iter()
                    .map(|text| ContentKind::Text { text })
                    .collect(),
            )),
            ContentHelper::SingleObject(content_type) => Ok(Content::Single(content_type)),
            ContentHelper::MultipleObjects(content_types) => Ok(Content::Multiple(content_types)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    id: Uuid,
    thread_id: Uuid,
    content: Content,
}

impl Message {
    pub fn id(&self) -> Uuid {
        self.id
    }
}

#[derive(Serialize, Deserialize)]
pub struct CreateMessage {
    content: Content,
}

impl CreateMessage {
    pub fn into_message(self, thread_id: Uuid) -> Message {
        Message {
            id: Uuid::new_v4(),
            thread_id,
            content: self.content,
        }
    }
}
