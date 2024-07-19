use rhai::{
    serde::{from_dynamic, to_dynamic},
    Dynamic, Engine, NativeCallContext,
};

fn send_sms(_number: String, _sms_msg: String) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let s = serde_json::json!({
        "status":"ok"
    });
    to_dynamic(s)
}

fn send_msg(_room_id: i64, _sms_msg: String) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let s = serde_json::json!({
        "status":"ok"
    });
    to_dynamic(s)
}

fn get_weather() -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let s = serde_json::json!({
        "status":"ok",
        "temp":"18",
        "weather":"雨"
    });
    to_dynamic(s)
}

fn get_current_time(_context: &NativeCallContext) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let time = std::time::SystemTime::now();
    let s = serde_json::json!({
        "status":"ok",
        "time": time
    });
    to_dynamic(s)
}

pub fn new_rhai() -> Engine {
    let mut engine = Engine::new();
    engine
        .register_fn("send_sms", send_sms)
        .register_fn("send_msg", send_msg)
        .register_fn("get_weather", get_weather)
        .register_fn("get_current_time", get_current_time);
    engine
}

impl super::ScriptEngin for Engine {
    fn eval(&self, code: &str) -> Result<String, String> {
        let r = self
            .eval::<rhai::Dynamic>(&code)
            .and_then(|d| from_dynamic::<serde_json::Value>(&d));
        match r {
            Ok(s) => Ok(s.to_string()),
            Err(err) => Err(err.to_string()),
        }
    }
}
