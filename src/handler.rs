use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Pointer};
use std::future::Future;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::Arc;

use futures::{FutureExt, TryFutureExt};
use futures::never::Never;
use hyper::{Body, Method, Response, Server};
use routerify::{Route, RouterBuilder, RouterService};
use serde::Serialize;

use crate::database::Database;
use crate::handler::RouteOption::FinishWithError;

pub struct AppInner {
    pub db: Database,
}

pub struct Frontend {
    pub app: App,
    pub router_builder: RouterBuilder<Body, Never>,
}

impl Frontend {
    pub fn get<P: Into<String>,
        O: Send + 'static + Future<Output=Result<Response<Body>, Never>>,
        H: Send + Sync + Clone + 'static + Fn(App, Request) -> O>(self, path: P, handler: H) -> Frontend {
        return Frontend {
            app: self.app.clone(),
            router_builder: self.router_builder.get(path, move |req| {
                let app = self.app.clone();
                let h = handler.clone();
                async move {
                    h(app, req).await
                }
            }),
        };
    }

    pub fn post<P: Into<String>,
        O: Send + 'static + Future<Output=Result<Response<Body>, Never>>,
        H: Send + Sync + Clone + 'static + Fn(App, Request) -> O>(self, path: P, handler: H) -> Frontend {
        return Frontend {
            app: self.app.clone(),
            router_builder: self.router_builder.post(path, move |req| {
                let app = self.app.clone();
                let h = handler.clone();
                async move {
                    h(app, req).await
                }
            }),
        };
    }

    pub fn delete<P: Into<String>,
        O: Send + Sync + 'static + Future<Output=Result<Response<Body>, Never>>,
        H: Send + Sync + Clone + 'static + Fn(App, Request) -> O>(self, path: P, handler: H) -> Frontend {
        return Frontend {
            app: self.app.clone(),
            router_builder: self.router_builder.delete(path, move |req| {
                let app = self.app.clone();
                let h = handler.clone();
                async move {
                    h(app, req).await
                }
            }),
        };
    }

    pub async fn launch(self) -> hyper::Result<()> {
        // Create a Service from the router above to handle incoming requests.
        let service = RouterService::new(self.router_builder.build().unwrap()).unwrap();

        // The address on which the server will be listening.
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

        // Create a server by passing the created service to `.serve` method.
        let server = Server::bind(&addr).serve(service);

        server.await
    }
}

#[derive(Clone)]
pub struct App {
    inner: Arc<AppInner>,
}

pub type Request = hyper::Request<Body>;

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

// pub trait RouteFuture<I, FutureIn: Future<Output=(Request, I)>> {
//     pub app: App,
//     pub inp: FutureIn,
// }

pub enum RouteOption<T> {
    Some(T),
    FinishWithError((String, u16)),
}

pub struct ThrowingRouteFuture<I, FutureIn: Future<Output=RouteOption<(Request, I)>>> {
    pub app: App,
    pub inp: FutureIn,
}


pub struct InitialRouteFuture {
    pub app: App,
}

pub fn receive(req: Request, app: App) -> ThrowingRouteFuture<(), impl Future<Output=RouteOption<(Request, ())>>> {
    return ThrowingRouteFuture {
        app: app,
        inp: async { RouteOption::Some((req, ())) },
    };
}

impl<I, F: Future<Output=RouteOption<(Request, I)>>> ThrowingRouteFuture<I, F> {
    // pub fn then<O, E,
    //     Ft: Future<Output=(Request, Result<O, E>)>,
    //     T: FnOnce(App, Request, I) -> Ft>
    // (self, func: T) -> ThrowingRouteFuture<Result<O, E>, impl Future<Output=RouteOption<(Request, Result<O, E>)>>> {
    //     let app = self.app.clone();
    //     let inp2 = self.inp.then(move |pre| async {
    //         match pre {
    //             RouteOption::Some((req, pre)) => {
    //                 RouteOption::Some(func(app, req, pre).await)
    //             }
    //             RouteOption::FinishWithError(e) => {
    //                 RouteOption::FinishWithError(e)
    //             }
    //         }
    //     });
    //     return ThrowingRouteFuture {
    //         app: self.app,
    //         inp: inp2,
    //     };
    // }
    pub fn then<O,
        Ft: Future<Output=(Request, O)>,
        T: FnOnce(App, Request, I) -> Ft>
    (self, func: T) -> ThrowingRouteFuture<O, impl Future<Output=RouteOption<(Request, O)>>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |pre| async {
            match pre {
                RouteOption::Some((req, pre)) => {
                    RouteOption::Some(func(app, req, pre).await)
                }
                RouteOption::FinishWithError(e) => {
                    RouteOption::FinishWithError(e)
                }
            }
        });
        return ThrowingRouteFuture {
            app: self.app,
            inp: inp2,
        };
    }
}

impl<I, E, F: Future<Output=RouteOption<(Request, Result<I, E>)>>> ThrowingRouteFuture<Result<I, E>, F> {
    /// Closure passed to this function will be called when a previous step fails with `Result::Err`.
    /// The stream will stop after this step and respond with the returned `String`, and status code.
    pub fn catch<
        Ft: Future<Output=(String, u16)>,
        T: FnOnce(E) -> Ft>
    (self, func: T) -> ThrowingRouteFuture<I, impl Future<Output=RouteOption<(Request, I)>>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |inp| async {
            match inp {
                RouteOption::Some((req, pre)) => {
                    match pre {
                        Ok(ok) => {
                            return RouteOption::Some((req, ok));
                        }
                        Err(err) => {
                            return FinishWithError(func(err).await);
                        }
                    }
                }
                RouteOption::FinishWithError((err, code)) => { return FinishWithError((err, code)); }
            }
        });
        return ThrowingRouteFuture {
            app: app,
            inp: inp2,
        };
    }
}

impl<I, F: Future<Output=RouteOption<(Request, I)>>> ThrowingRouteFuture<I, F> where I: Into<Body> {
    pub async fn finish(self) -> Result<Response<Body>, Never> {
        match self.inp.await {
            RouteOption::Some((_, res)) => {
                Ok(Response::builder().status(200).body(res.into()).unwrap())
            }
            RouteOption::FinishWithError((err, code)) => {
                Ok(Response::builder().status(code).body(err.into()).unwrap())
            }
        }
    }
}

pub trait Responder {
    fn respond(self) -> Result<String, &'static str>;
}

// impl<A: Serialize, B: Error> Responder for Result<Option<A>, B> {
//     fn respond(self) -> Result<String, &'static str> {
//         match self {
//             Ok(Some(user)) => {
//                 return Result::Ok(serde_json::to_string(&user).unwrap());
//             }
//             Ok(None) => {
//                 return Result::Err(("Not Found", 404));
//             }
//             Err(e) => {
//                 return Result::Err(("Database error", 500));
//             }
//         }
//     }
// }

pub trait Combine<First, Error> {
    fn adding<Second>(self, second: Second) -> Result<(First, Second), Error>;
}

impl<First, Error> Combine<First, Error> for Result<First, Error> {
    fn adding<Second>(self, second: Second) -> Result<(First, Second), Error> {
        match self {
            Ok(first) => {
                Ok((first, second))
            }
            Err(err) => {
                Err(err)
            }
        }
    }
}