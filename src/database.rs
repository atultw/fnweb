use futures::TryStreamExt;
use mongodb::{bson, Client};
use mongodb::bson::Document;
use mongodb::options::ClientOptions;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub struct Database {
    pub client: mongodb::Client,
}

impl Database {

    pub async fn new(url: &str) -> Self {
        let mut client_options =
            ClientOptions::parse(url)
                .await.expect("Failed to connect to data layer");
        // Manually set an option
        client_options.app_name = Some("Demo".to_string());
        // Get a handle to the cluster
        let client = Client::with_options(client_options).unwrap();
        return Self {
            client
        }
    }

    pub async fn retrieve_one<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: Document) -> Result<Option<T>, mongodb::error::Error> {
        let result = self.client
            .database("app")
            .collection(&*table_name)
            .find_one(Some(filter), None)
            .await;

        match result {
            Ok(resp) => {
                match resp {
                    None => { Ok(None) }
                    Some(doc) => {
                        match bson::from_bson(bson::Bson::Document(doc)) {
                            Ok(o) => { Ok(o) }
                            Err(err) => { Err(mongodb::error::Error::from(err)) }
                        }
                    }
                }
            }
            Err(bad) => {
                Err(bad)
            }
        }
    }

    pub async fn retrieve_many<T: Send + Serialize + DeserializeOwned>(&self, table_name: String, filter: Document) -> Result<Vec<T>, mongodb::error::Error> {
        let mut cursor = self.client
            // TODO: configurable db name
            .database("app")
            .collection(&*table_name)
            .find(Some(filter), None)
            .await.unwrap();

        let mut res: Vec<T> = vec![];

        while let Ok(Some(item)) = cursor.try_next().await {
            match bson::from_bson(bson::Bson::Document(item)) {
                Ok(loaded) => {
                    res.push(loaded);
                }
                Err(err) => {
                    return Err(mongodb::error::Error::from(err));
                }
            }
        }
        return Ok(res);
    }
}