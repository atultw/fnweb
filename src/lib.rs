mod handler;
mod database;

#[cfg(test)]
mod tests {
    use std::fmt::{Debug, Error};
    use crate::handler::Combine;

    use hyper::{Body, Response};
    use mongodb::bson::doc;
    use routerify::prelude::RequestExt;
    use serde::{Deserialize, Serialize};

    use crate::database::Database;
    use crate::handler::{App, Frontend, receive, Request, Responder};

    #[derive(Serialize, Deserialize)]
    struct User {
        pub name: String,
    }

    async fn auth_filter(_: App, req: &Request) -> Result<User, String> {
        let user = User {
            name: String::from(req.headers().get("User-Agent").unwrap().to_str().unwrap())
        };
        Ok(user)
    }

    fn ip_filter(_: App, req: &Request) -> Result<String, String> {
        let ip = String::from(req.headers().get("X-Forwarded-For").unwrap().to_str().unwrap());
        Ok(ip)
    }

    #[tokio::test]
    async fn it_works() {
        // let get_user = move |app, req| async move {
        //     receive(req, app)
        //         .then(|a, req, _| async move {
        //             let id_str: Option<&String> = req.param("id");
        //             match id_str {
        //                 Some(oid) => {
        //                     let res = a
        //                         .database()
        //                         .retrieve_one::<User>(String::from("users"), doc! {"id":oid})
        //                         .await;
        //                     (req, res.map_err(|s| "database"))
        //                 }
        //                 None => {
        //                     return (req, Err("no id param"));
        //                 }
        //             }
        //         })
        //         .finish().await
        // };
        let hello = move |app, req| async move {
            receive(req, app)
                .then(|a, req, _| async move {
                    (req, auth_filter(a, &req).await)
                })
                .catch(|err| async move {
                    ("Auth error".to_string(), 401)
                })
                .then(|a, req, u| async move {
                    (req, ip_filter(a, &req).adding(u))
                })
                .catch(|err| async move {
                    ("Couldn't get your ip".to_string(), 400)
                })
                .then(|a, req, (ip, u)| async move {
                    (req, "Hello, ".to_owned() + u.name.as_str() + "! Your IP is:" + &*ip)
                })
                // .catch(|err| async move {
                //     ("unknown".to_string(), 500)
                // })
                .finish().await
        };
        let db = Database::new("mongodb://localhost:27017").await;
        let app = App::new_with_database(db);
        let frontend = Frontend { app, router_builder: Default::default() };
        frontend
            // .get("/user/:id", get_user)
            .get("/hello", hello)
            .launch().await.unwrap()
    }
}
