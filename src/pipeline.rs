use std::fmt::{Formatter, Pointer};
use std::future::Future;
use futures::future::{BoxFuture, Ready};
use futures::FutureExt;
use futures::never::Never;
use hyper::{Body, Response};
use crate::application::{App};


use crate::pipeline::RouteOption::FinishWithError;

pub type Request = hyper::Request<Body>;

pub enum RouteOption<T> {
    Some(T),
    FinishWithError((String, u16)),
}

pub struct Pipeline<I, FutureIn: Future<Output=RouteOption<I>>> {
    pub app: App,
    pub inp: FutureIn,
}

impl Pipeline<Request, Ready<RouteOption<Request>>> {
    pub fn receive(req: Request, app: App) -> Pipeline<Request, impl Future<Output=RouteOption<Request>>> {
        return Pipeline {
            app: app,
            inp: futures::future::ready(RouteOption::Some(req)),
        };
    }
}

impl<I, F: Future<Output=RouteOption<I>>> Pipeline<I, F> {
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
    pub fn then<
        O,
        Ft: Future<Output=O> + Send,
        T: FnOnce(App, I) -> Ft + Send>
    (self, func: T) -> Pipeline<O, impl Future<Output=RouteOption<O>>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |pre| async {
            match pre {
                RouteOption::Some(pre) => {
                    let fut = func(app, pre).await;
                    RouteOption::Some(fut)
                }
                RouteOption::FinishWithError(e) => {
                    RouteOption::FinishWithError(e)
                }
            }
        });
        return Pipeline {
            app: self.app,
            inp: inp2,
        };
    }
}

impl<I, E, F: Future<Output=RouteOption<Result<I, E>>>> Pipeline<Result<I, E>, F> {
    /// Closure passed to this function will be called when a previous step fails with `Result::Err`.
    /// The stream will stop after this step and respond with the returned `String`, and status code.
    pub fn catch_async<
        S: Into<String>,
        Ft: Future<Output=(S, u16)>,
        T: FnOnce(E) -> Ft>
    (self, func: T) -> Pipeline<I, impl Future<Output=RouteOption<I>>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |inp| async {
            match inp {
                RouteOption::Some(pre) => {
                    match pre {
                        Ok(ok) => {
                            return RouteOption::Some(ok);
                        }
                        Err(err) => {
                            let ret = func(err).await;
                            return FinishWithError((ret.0.into(), ret.1));
                        }
                    }
                }
                RouteOption::FinishWithError((err, code)) => { return FinishWithError((err, code)); }
            }
        });
        return Pipeline {
            app: app,
            inp: inp2,
        };
    }
}

impl<I, E, F: Future<Output=RouteOption<Result<I, E>>>> Pipeline<Result<I, E>, F> {
    /// Closure passed to this function will be called when a previous step fails with `Result::Err`.
    /// The stream will stop after this step and respond with the returned `String`, and status code.
    pub fn catch<
        S: Into<String>,
        T: FnOnce(E) -> (S, u16)>
    (self, func: T) -> Pipeline<I, impl Future<Output=RouteOption<I>>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |inp| async {
            match inp {
                RouteOption::Some(pre) => {
                    match pre {
                        Ok(ok) => {
                            return RouteOption::Some(ok);
                        }
                        Err(err) => {
                            let ret = func(err);
                            return FinishWithError((ret.0.into(), ret.1));
                        }
                    }
                }
                RouteOption::FinishWithError((err, code)) => { return FinishWithError((err, code)); }
            }
        });
        return Pipeline {
            app: app,
            inp: inp2,
        };
    }
}

impl<I, F: Future<Output=RouteOption<Option<I>>>> Pipeline<Option<I>, F> {
    /// Closure passed to this function will be called when a previous step returns `Option::None`.
    /// The stream will stop after this step, responding with the returned `String` and status code.
    pub fn if_none(self, message: (String, u16)) -> Pipeline<I, impl Future<Output=RouteOption<I>>> {
        let app = self.app.clone();
        let inp2 = self.inp.then(move |inp| async {
            match inp {
                RouteOption::Some(pre) => {
                    match pre {
                        Some(ok) => {
                            return RouteOption::Some(ok);
                        }
                        None => {
                            return FinishWithError(message);
                        }
                    }
                }
                RouteOption::FinishWithError((err, code)) => { return FinishWithError((err, code)); }
            }
        });
        return Pipeline {
            app: app,
            inp: inp2,
        };
    }
}

impl<I, F: Future<Output=RouteOption<I>>> Pipeline<I, F> where I: Into<Body> {
    pub async fn finish(self) -> Result<Response<Body>, Never> {
        match self.inp.await {
            RouteOption::Some(res) => {
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