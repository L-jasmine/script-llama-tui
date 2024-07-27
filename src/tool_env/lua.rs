use mlua::prelude::*;

pub fn new_lua() -> Result<Lua, LuaError> {
    let lua = Lua::new();

    let send_sms = lua.create_function(
        |lua, (number, sms_msg): (String, String)| -> LuaResult<mlua::Table> {
            let r = lua.create_table()?;
            r.set("status", "ok")?;
            r.set("number", number)?;
            r.set("sms_msg", sms_msg)?;
            Ok(r)
        },
    )?;

    let send_msg = lua.create_function(
        |lua, (room_id, message): (u64, String)| -> LuaResult<mlua::Table> {
            let r = lua.create_table()?;
            r.set("status", "ok")?;
            r.set("room_id", room_id)?;
            r.set("message", message)?;
            Ok(r)
        },
    )?;

    let remember = lua.create_function(
        |lua, (_time, _text): (u64, String)| -> LuaResult<mlua::Table> {
            let r = lua.create_table()?;
            r.set("status", "ok")?;
            Ok(r)
        },
    )?;

    let get_weather = lua.create_function(|lua, _: ()| -> LuaResult<mlua::Table> {
        let r = lua.create_table()?;
        r.set("status", "ok")?;
        r.set("temp", "18")?;
        r.set("weather", "雨")?;
        Ok(r)
    })?;

    let get_current_time = lua.create_function(|lua, _: ()| -> LuaResult<mlua::Table> {
        let time = chrono::Local::now().to_rfc3339();
        let r = lua.create_table()?;
        r.set("status", "ok")?;
        r.set("time", time)?;
        Ok(r)
    })?;

    lua.globals().set("send_sms", send_sms)?;
    lua.globals().set("send_msg", send_msg)?;
    lua.globals().set("remember", remember)?;
    lua.globals().set("get_weather", get_weather)?;
    lua.globals().set("get_current_time", get_current_time)?;

    Ok(lua)
}

impl super::ScriptEngin for Lua {
    fn eval(&self, code: &str) -> Result<String, String> {
        self.load(code)
            .eval::<mlua::Value>()
            .map_err(|e| e.to_string())
            .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
    }
}
