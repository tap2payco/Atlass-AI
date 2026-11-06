use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use platform_contracts::execution::ExecutionContext;
use crate::pipeline::CoreResult;

#[async_trait]
pub trait Middleware<I, O>: Send + Sync {
    async fn handle(&self, input: I, context: &ExecutionContext, next: &dyn Next<I, O>) -> CoreResult<O>;
}

#[async_trait]
pub trait Next<I, O>: Send + Sync {
    async fn run(&self, input: I, context: &ExecutionContext) -> CoreResult<O>;
}

pub struct MiddlewareStack<I, O> {
    middlewares: Vec<Arc<dyn Middleware<I, O>>>,
}

impl<I: Send + Sync + 'static, O: Send + Sync + 'static> MiddlewareStack<I, O> {
    pub fn new() -> Self {
        Self { middlewares: Vec::new() }
    }

    pub fn with_middleware(mut self, middleware: Arc<dyn Middleware<I, O>>) -> Self {
        self.middlewares.push(middleware);
        self
    }

    pub async fn execute<F, Fut>(&self, input: I, context: &ExecutionContext, core_handler: F) -> CoreResult<O>
    where
        F: Fn(I, &ExecutionContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = CoreResult<O>> + Send + 'static,
    {
        struct CoreNext<F, I, O> {
            handler: F,
            _marker: std::marker::PhantomData<(I, O)>,
        }

        #[async_trait]
        impl<F, Fut, I, O> Next<I, O> for CoreNext<F, I, O>
        where
            F: Fn(I, &ExecutionContext) -> Fut + Send + Sync,
            Fut: std::future::Future<Output = CoreResult<O>> + Send,
            I: Send + Sync,
            O: Send + Sync,
        {
            async fn run(&self, input: I, context: &ExecutionContext) -> CoreResult<O> {
                (self.handler)(input, context).await
            }
        }

        let core_next = CoreNext {
            handler: core_handler,
            _marker: std::marker::PhantomData,
        };

        if self.middlewares.is_empty() {
            return core_next.run(input, context).await;
        }

        let mut runner: Box<dyn Next<I, O>> = Box::new(core_next);
        for mw in self.middlewares.iter().rev() {
            runner = Box::new(MiddlewareRunner {
                middleware: mw.clone(),
                next: runner,
            });
        }

        runner.run(input, context).await
    }
}

impl<I: Send + Sync + 'static, O: Send + Sync + 'static> Default for MiddlewareStack<I, O> {
    fn default() -> Self {
        Self::new()
    }
}

struct MiddlewareRunner<I, O> {
    middleware: Arc<dyn Middleware<I, O>>,
    next: Box<dyn Next<I, O>>,
}

#[async_trait]
impl<I: Send + Sync, O: Send + Sync> Next<I, O> for MiddlewareRunner<I, O> {
    async fn run(&self, input: I, context: &ExecutionContext) -> CoreResult<O> {
        self.middleware.handle(input, context, self.next.as_ref()).await
    }
}

pub struct TimeoutMiddleware {
    pub timeout: Duration,
}

#[async_trait]
impl<I: Send + Sync + 'static, O: Send + Sync + 'static> Middleware<I, O> for TimeoutMiddleware {
    async fn handle(&self, input: I, context: &ExecutionContext, next: &dyn Next<I, O>) -> CoreResult<O> {
        tokio::select! {
            res = next.run(input, context) => res,
            _ = tokio::time::sleep(self.timeout) => Err("Execution timed out in Middleware".to_string()),
        }
    }
}
