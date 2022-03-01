use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Pointer};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::FutureExt;
use futures::never::Never;
use hyper::{Body, Method, Response, Server};
use routerify::{RouterBuilder, RouterService};
use serde::Serialize;

use crate::database::Database;

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

pub struct RouteFuture<I, E: Into<String>, FutureIn: Future<Output=(Request, RouteResult<I, E>)>> {
    pub app: App,
    pub inp: FutureIn,
}

pub struct RouteFutureFirst {
    pub app: App,
}

pub enum RouteResult<T, E: Into<String>> {
    Ok(T),
    Err((E, u16)),
}

pub fn receive<E: Into<String>>(req: Request, app: App) -> RouteFuture<(), E, impl Future<Output=(Request, RouteResult<(), E>)>> {
    return RouteFuture {
        app: app,
        inp: (|| async { (req, RouteResult::Ok(())) })(),
    };
}

impl<I, E: Into<String>, F: Future<Output=(Request, RouteResult<I, E>)>> RouteFuture<I, E, F> {
    pub fn run<O,
        Ft: Future<Output=(Request, RouteResult<O, E>)>,
        T: FnOnce(App, Request, I) -> Ft>
    (self, func: T) -> RouteFuture<O, E, impl Future<Output=(Request, RouteResult<O, E>)>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |inpp| {
            let app = self.app.clone();
            async move {
                match inpp.1 {
                    RouteResult::Ok(inn) => {
                        (func(app, inpp.0, inn).await)
                    }
                    RouteResult::Err(err) => {
                        (inpp.0, RouteResult::Err(err))
                    }
                }
            }
        });
        return RouteFuture {
            app: app,
            inp: inp2,
        };
    }
}

impl<I, E: Into<String>, F: Future<Output=(Request, RouteResult<I, E>)>> RouteFuture<I, E, F> where I: Into<Body> {
    pub async fn finish(self) -> Result<Response<Body>, Never> {
        match self.inp.await.1 {
            RouteResult::Ok(ok) => { Ok(Response::builder().status(200).body(ok.into()).unwrap()) }
            // TODO: real status
            RouteResult::Err(err) => { Ok(Response::builder().status(err.1).body(Body::from(err.0.into())).unwrap()) }
        }
    }
}

pub trait Responder {
    fn respond(self) -> RouteResult<String, &'static str>;
}

impl<A: Serialize, B: Error> Responder for Result<Option<A>, B> {
    fn respond(self) -> RouteResult<String, &'static str> {
        match self {
            Ok(Some(user)) => {
                return RouteResult::Ok(serde_json::to_string(&user).unwrap());
            }
            Ok(None) => {
                return RouteResult::Err(("Not Found", 404));
            }
            Err(e) => {
                return RouteResult::Err(("Database error", 500));
            }
        }
    }
}
