use mlua::prelude::*;

pub fn new_lua() -> Result<Lua, LuaError> {
    let lua = Lua::new();

    let send_sms = lua.create_function(
        |_, (number, sms_msg): (String, String)| -> LuaResult<String> {
            // println!("lua: Sending SMS to {}: {}", number, sms_msg);

            let s = serde_json::json!({
                "status":"ok"
            })
            .to_string();

            Ok(s)
        },
    )?;

    let send_msg = lua.create_function(
        |_, (room_id, message): (u64, String)| -> LuaResult<String> {
            // println!("lua: Sending message to room {}: {}", room_id, message);

            let s = serde_json::json!({
                "status":"ok"
            })
            .to_string();

            Ok(s)
        },
    )?;

    // set_timer(time:int,text:string) // 这个函数可以设置一个定时器，time是时间间隔(s)，func是回调函数
    let remember = lua.create_function(|_, (time, text): (u64, String)| -> LuaResult<String> {
        println!("set_timer {time}: {text}");
        let s = serde_json::json!({
            "status":"ok"
        })
        .to_string();
        Ok(s)
    })?;

    let get_weather = lua.create_function(|_, _: ()| -> LuaResult<String> {
        println!("get_weather");
        Ok("下雨".to_string())
    })?;

    lua.globals().set("send_sms", send_sms)?;
    lua.globals().set("send_msg", send_msg)?;
    lua.globals().set("remember", remember)?;
    lua.globals().set("get_weather", get_weather)?;

    Ok(lua)
}
