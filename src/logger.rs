use crate::ui_types::{NotificationKind, WorkerUpdate};
use tokio::sync::mpsc::Sender;
use tracing::field::Visit;
use tracing_subscriber::Layer;

pub struct GuiLogger {
    pub tx: Sender<WorkerUpdate>,
}

struct MsgVisitor {
    msg: String,
}

impl Visit for MsgVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.msg = format!("{:?}", value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.msg = value.to_string();
        }
    }
}

impl<S> Layer<S> for GuiLogger
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let target = metadata.target();

        // Anti-recursion / Performance Guard: Filter out tracing/scheduling internals
        if target.starts_with("tokio") || target.starts_with("runtime") || target.starts_with("eframe") || target.starts_with("egui") {
            return;
        }

        let mut visitor = MsgVisitor { msg: String::new() };
        event.record(&mut visitor);

        if visitor.msg.is_empty() {
            return;
        }

        let kind = match *metadata.level() {
            tracing::Level::ERROR => NotificationKind::Error,
            tracing::Level::WARN => NotificationKind::Warning,
            tracing::Level::INFO => NotificationKind::Info,
            tracing::Level::DEBUG => NotificationKind::Debug,
            tracing::Level::TRACE => NotificationKind::Trace,
        };

        // Try-send ensures standard synchronous logging won't block asynchronous execution loops
        let _ = self.tx.try_send(WorkerUpdate::Notify {
            message: visitor.msg,
            kind,
        });
    }
}