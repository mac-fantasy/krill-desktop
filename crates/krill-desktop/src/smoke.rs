use crate::{app::Dashboard, updates};
use gpui_kit::{AnyWindowHandle, App, Entity};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

// Runs inside the real native event loop and renderer, using synthetic data only.
// Pointer/key behavior is exercised separately by GPUI interaction tests.
pub fn run(
    window: AnyWindowHandle,
    view: Entity<Dashboard>,
    output: PathBuf,
    success: Arc<AtomicBool>,
    cx: &mut App,
) {
    view.update(cx, |view, _| {
        view.state.preferences.panel_pinned = true;
        view.state.preferences.selected_subscription_id = Some(101);
    });
    cx.spawn(async move |cx|{
        let mut tabs=Vec::new();
        for tab in 0..5 {
            view.update(cx,|view,cx|view.switch_tab(tab,cx));
            cx.background_executor().timer(Duration::from_millis(300)).await;
            if let Ok(bounds)=window.update(cx,|_,window,_|window.bounds()) {
                tabs.push(serde_json::json!({"tab":tab,"width":f32::from(bounds.size.width),"height":f32::from(bounds.size.height)}));
            }
        }
        let account=view.update(cx,|view,_|view.state.is_demo && !view.state.needs_login && view.state.subscriptions.is_some() && view.state.primary_remaining()=="$72.60");
        let valid=account && tabs.len()==5 && tabs.iter().all(|t|t["width"].as_f64().unwrap_or(0.)>=430. && t["height"].as_f64().unwrap_or(0.)>=500.);
        let report=serde_json::json!({"success":valid,"mode":"synthetic_native_event_loop","version":updates::CURRENT_VERSION,"platform":updates::PLATFORM,"tabs":tabs,"account_projection":account});
        if std::fs::create_dir_all(&output).is_ok() && std::fs::write(output.join("verification.json"),serde_json::to_vec_pretty(&report).unwrap()).is_ok() {success.store(valid,Ordering::Release);}
        let _=cx.update(|cx|cx.quit());
    }).detach();
}
