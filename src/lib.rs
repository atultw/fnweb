mod handler;
mod database;

#[cfg(test)]
mod tests {
    use std::fmt::{Debug, Error};

    use hyper::{Body, Response};

    use mongodb::bson::doc;
    use routerify::prelude::RequestExt;
    use crate::database::Database;

    use crate::handler::{App, Frontend, receive, Request, Responder, RouteResult};
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    struct User {
        pub name: String
    }
    async fn auth_filter(_: App, req: Request) -> (Request, RouteResult<User, String>) {
        let user = User{
            name: String::from(req.headers().get("User-Agent").unwrap().to_str().unwrap())
        };
        (req, RouteResult::Ok(user))
    }

    async fn ip_filter(_: App, req: Request) -> (Request, RouteResult<String, String>) {
       let ip = String::from(req.headers().get("X-Forwarded-For").unwrap().to_str().unwrap());
        (req, RouteResult::Ok(ip))
    }

    #[tokio::test]
    async fn it_works() {
        let get_user = move |app, req| async move {
            receive(req, app)
                .run(|a, req, _| async move {
                    let id_str: Option<&String> = req.param("id");
                    match id_str {
                        Some(oid) => {
                            let res = a
                                .database()
                                .retrieve_one::<User>(String::from("users"), doc!{"id":oid})
                                .await
                                .respond();
                            (req, res)
                        }
                        None => {
                            return (req, RouteResult::Err(("Missing id param", 400)))
                        }
                    }

                })
                .finish().await
        };
        let hello = move |app, req| async move {
            receive(req, app)
                .run(|a, req, _| async move {
                    auth_filter(a, req).await
                })
                .run(|a, req, u| async move {
                    (u, ip_filter(a, req).await)
                })
                .run(|a, req, u, ip| async move {
                    (req, RouteResult::Ok("Hello, ".to_owned() + u.name.as_str() + "! Your IP is:" + ip))
                })
                .finish().await
        };
        let db = Database::new("mongodb://localhost:27017").await;
        let app = App::new_with_database(db);
        let frontend = Frontend { app, router_builder: Default::default() };
        frontend
            .get("/user/:id", get_user)
            .get("/hello", hello)
            .launch().await.unwrap()
    }
}
