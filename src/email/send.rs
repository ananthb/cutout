//! Send outbound emails via the Cloudflare EMAIL binding.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use worker::*;

use crate::types::OutboundEmail;

#[wasm_bindgen]
extern "C" {
    pub type SendEmailBinding;

    #[wasm_bindgen(method)]
    fn send(this: &SendEmailBinding, message: &JsValue) -> js_sys::Promise;
}

/// Send a single outbound email through the EMAIL binding.
pub async fn send_outbound(env: &Env, outbound: &OutboundEmail) -> Result<()> {
    let env_js: JsValue = env.clone().into();
    let binding = js_sys::Reflect::get(&env_js, &"EMAIL".into())
        .map_err(|_| Error::from("Missing EMAIL binding"))?;
    let send_email: SendEmailBinding = binding.unchecked_into();

    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"from".into(), &JsValue::from_str(&outbound.from))
        .map_err(|_| Error::from("set from"))?;
    js_sys::Reflect::set(&obj, &"to".into(), &JsValue::from_str(&outbound.to))
        .map_err(|_| Error::from("set to"))?;
    let uint8 = js_sys::Uint8Array::from(outbound.raw.as_slice());
    js_sys::Reflect::set(&obj, &"raw".into(), &uint8.into()).map_err(|_| Error::from("set raw"))?;

    let promise = send_email.send(&obj.into());
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| Error::from(format!("send_email failed: {e:?}")))?;
    Ok(())
}
