use jni::{
    objects::{JClass, JString},
    sys::jstring,
    JNIEnv,
};
use serde_json::{json, Value};
#[no_mangle]
pub extern "system" fn Java_org_questjellyfin_player_MainActivity_nativeRequest(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    input: JString<'_>,
) -> jstring {
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Value, String> {
            let request: String = env
                .get_string(&input)
                .map_err(|_| "Invalid request")?
                .into();
            let value = serde_json::from_str(&request).map_err(|_| "Invalid request")?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| "Cannot start runtime")?;
            runtime.block_on(crate::bridge::run(value))
        }));
    let value = match result {
        Ok(Ok(data)) => json!({"ok":true,"data":data}),
        Ok(Err(message)) => json!({"ok":false,"error":message}),
        Err(_) => json!({"ok":false,"error":"Internal error"}),
    };
    env.new_string(value.to_string())
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
