use super::client::{realtime_ws_url, SupabaseClient};
use super::push;
use super::worker::{SyncSnapshot, Wake};
use crate::error::AppResult;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Pull,
    Full,
}

pub fn wake_for_message(raw: &str) -> Option<Trigger> {
    let value: Value = serde_json::from_str(raw).ok()?;
    let event = value["event"].as_str().unwrap_or("");
    match event {
        "postgres_changes" | "INSERT" | "UPDATE" | "DELETE" => Some(Trigger::Pull),
        "phx_error" | "phx_close" => Some(Trigger::Full),
        _ => {
            let kind = value["payload"]["data"]["type"].as_str().unwrap_or("");
            if matches!(kind, "INSERT" | "UPDATE" | "DELETE") {
                Some(Trigger::Pull)
            } else {
                None
            }
        }
    }
}

pub fn spawn(client: Arc<SupabaseClient>, wakes: Sender<Wake>, snapshot: Arc<Mutex<SyncSnapshot>>) {
    tauri::async_runtime::spawn(async move {
        run_loop(client, wakes, snapshot).await;
    });
}

async fn run_loop(client: Arc<SupabaseClient>, wakes: Sender<Wake>, snapshot: Arc<Mutex<SyncSnapshot>>) {
    let mut attempt = 0u32;
    loop {
        match connect_once(client.clone(), &wakes, &snapshot).await {
            Ok(()) => attempt = 0,
            Err(_) => {
                set_realtime(&snapshot, false);
                attempt = attempt.saturating_add(1);
                tokio::time::sleep(push::backoff(attempt)).await;
            }
        }
    }
}

async fn connect_once(
    client: Arc<SupabaseClient>,
    wakes: &Sender<Wake>,
    snapshot: &Arc<Mutex<SyncSnapshot>>,
) -> AppResult<()> {
    let (url, token) = tauri::async_runtime::spawn_blocking({
        let client = client.clone();
        move || {
            let url = realtime_ws_url(client.project_url(), client.anon_key());
            let token = client.access_token()?;
            Ok::<_, crate::error::AppError>((url, token))
        }
    })
    .await
    .map_err(|_| crate::error::AppError::storage("No se pudo abrir Realtime"))??;

    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(|_| crate::error::AppError::storage("No se pudo abrir Realtime"))?;
    let (mut write, mut read) = ws.split();
    write
        .send(Message::Text(join_payload(&token).into()))
        .await
        .map_err(|_| crate::error::AppError::storage("No se pudo abrir Realtime"))?;
    set_realtime(snapshot, true);
    let _ = wakes.send(Wake::Full);

    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    let mut ref_n = 2u64;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                ref_n += 1;
                let beat = json!({"topic":"phoenix","event":"heartbeat","payload":{},"ref": ref_n.to_string()});
                if write.send(Message::Text(beat.to_string().into())).await.is_err() {
                    set_realtime(snapshot, false);
                    return Err(crate::error::AppError::storage("Realtime desconectado"));
                }
            }
            next = read.next() => {
                match next {
                    Some(Ok(Message::Text(text))) => {
                        match wake_for_message(&text) {
                            Some(Trigger::Pull) => { let _ = wakes.send(Wake::Pull); }
                            Some(Trigger::Full) => { let _ = wakes.send(Wake::Full); }
                            None => {}
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        set_realtime(snapshot, false);
                        return Err(crate::error::AppError::storage("Realtime desconectado"));
                    }
                    Some(Err(_)) => {
                        set_realtime(snapshot, false);
                        return Err(crate::error::AppError::storage("Realtime desconectado"));
                    }
                    _ => {}
                }
            }
        }
    }
}

fn join_payload(access_token: &str) -> String {
    let tables = ["rooms", "rate_plans", "products", "business_settings", "app_users", "catalog_deletes"];
    let changes: Vec<Value> = tables
        .iter()
        .map(|table| json!({"event":"*","schema":"public","table": table}))
        .collect();
    json!({
        "topic": "realtime:nightdesk-catalog",
        "event": "phx_join",
        "payload": {
            "config": {
                "broadcast": {"ack": false, "self": false},
                "presence": {"key": ""},
                "postgres_changes": changes
            },
            "access_token": access_token
        },
        "ref": "1",
        "join_ref": "1"
    })
    .to_string()
}

fn set_realtime(snapshot: &Arc<Mutex<SyncSnapshot>>, connected: bool) {
    if let Ok(mut guard) = snapshot.lock() {
        guard.realtime_connected = connected;
        guard.connected = connected || guard.last_push_at.is_some() || guard.last_pull_at.is_some();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_changes_wakes_pull() {
        assert_eq!(
            wake_for_message(r#"{"event":"postgres_changes","payload":{}}"#),
            Some(Trigger::Pull)
        );
        assert_eq!(
            wake_for_message(r#"{"event":"INSERT","payload":{}}"#),
            Some(Trigger::Pull)
        );
        assert_eq!(
            wake_for_message(r#"{"event":"phx_reply","payload":{"status":"ok"}}"#),
            None
        );
        assert_eq!(
            wake_for_message(r#"{"event":"heartbeat","payload":{}}"#),
            None
        );
        assert_eq!(
            wake_for_message(r#"{"event":"phx_close","payload":{}}"#),
            Some(Trigger::Full)
        );
        assert_eq!(
            wake_for_message(r#"{"event":"phx_error","payload":{}}"#),
            Some(Trigger::Full)
        );
    }
}
