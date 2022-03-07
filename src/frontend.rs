use routerify::{RouterBuilder, RouterService};
use hyper::{Body, Response, Server};
use futures::never::Never;
use std::future::Future;
use std::net::SocketAddr;
use crate::application::App;
use crate::pipeline::Request;

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
