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
        #[cfg(feature="native-qa")]
        let mut captures_valid=true;
        #[cfg(not(feature="native-qa"))]
        let captures_valid=true;
        let _=std::fs::create_dir_all(&output);
        for tab in 0..5 {
            view.update(cx,|view,cx|view.switch_tab(tab,cx));
            cx.background_executor().timer(Duration::from_millis(300)).await;
            if let Ok(bounds)=window.update(cx,|_,window,_|window.bounds()) {
                tabs.push(serde_json::json!({"tab":tab,"width":f32::from(bounds.size.width),"height":f32::from(bounds.size.height)}));
            }
            #[cfg(feature="native-qa")]
            {
                let path=output.join(format!("tab-{tab}.png"));
                let capture=window.update(cx,|_,window,cx|{
                    window.refresh();
                    let _=window.draw(cx);
                    window.render_to_image().and_then(|image|image.save(&path).map_err(Into::into))
                });
                if !matches!(capture,Ok(Ok(()))){captures_valid=false;eprintln!("Failed to render native UI capture for tab {tab}");}
            }
        }
        let account=view.update(cx,|view,_|view.state.is_demo && !view.state.needs_login && view.state.subscriptions.is_some() && view.state.primary_remaining()=="$72.60");
        #[cfg(target_os="windows")]
        let orb=view.update(cx,|view,_|view.orb.clone()).map(|orb|orb.update(cx,|orb,cx|orb.verify_native(cx))).unwrap_or(serde_json::json!({"success":false}));
        #[cfg(not(target_os="windows"))]
        let orb=serde_json::json!({"success":true,"applicable":false});
        let browser=crate::browser::BrowserLogin::verify(cx).await.unwrap_or_else(|e|serde_json::json!({"success":false,"error":e.to_string()}));
        let valid=captures_valid && account && orb["success"]==true && browser["success"]==true && tabs.len()==5 && tabs.iter().all(|t|t["width"].as_f64().unwrap_or(0.)>=430. && t["height"].as_f64().unwrap_or(0.)>=500.);
        let report=serde_json::json!({"success":valid,"captures":cfg!(feature="native-qa") && captures_valid,"mode":"synthetic_native_event_loop","version":updates::CURRENT_VERSION,"platform":updates::PLATFORM,"tabs":tabs,"account_projection":account,"orb":orb,"browser":browser});
        if std::fs::create_dir_all(&output).is_ok() && std::fs::write(output.join("verification.json"),serde_json::to_vec_pretty(&report).unwrap()).is_ok() {success.store(valid,Ordering::Release);}
        if !valid{eprintln!("Native smoke verification failed; see verification.json");std::process::exit(1);}
        let _=cx.update(|cx|cx.quit());
    }).detach();
}
