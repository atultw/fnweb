use std::sync::Arc;
use crate::database::Database;

#[derive(Clone)]
pub struct App {
    inner: Arc<AppInner>,
}

pub struct AppInner {
    pub db: Database,
}

impl App {
    pub fn new_with_database(db: Database) -> Self {
        return Self {
            inner: Arc::new(AppInner {
                db
            })
        };
    }

    pub fn database(&self) -> &Database {
        &self.inner.db
    }
}


