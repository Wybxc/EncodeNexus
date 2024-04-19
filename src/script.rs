use std::collections::BTreeMap;
use std::sync::Arc;

use egui::mutex::Mutex;
use indexmap::IndexMap;
use mlua::prelude::*;
use once_cell::sync::Lazy;

use crate::node::{Control, NodePrototype, Pin};

pub static REGISTRY: Lazy<Mutex<BTreeMap<String, Arc<NodePrototype>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

pub static CATEGORY: Lazy<Mutex<BTreeMap<String, NodeEntry>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

pub enum NodeEntry {
    Node(Arc<NodePrototype>),
    Category(BTreeMap<String, NodeEntry>),
}

impl NodeEntry {
    pub fn menu(
        &self,
        name: &str,
        ui: &mut egui::Ui,
        new_node: &mut impl FnMut(Arc<NodePrototype>),
    ) {
        match self {
            NodeEntry::Node(prototype) => {
                if ui.button(name).clicked() {
                    new_node(prototype.clone());
                    ui.close_menu();
                }
            }
            NodeEntry::Category(dir) => {
                ui.menu_button(name, |ui| {
                    for (name, entry) in dir {
                        entry.menu(name, ui, new_node)
                    }
                });
            }
        }
    }
}

pub fn init_lua() -> LuaResult<Lua> {
    let lua = Lua::new();
    init_global(&lua)?;

    for script in glob::glob("scripts/**/*.luau").unwrap() {
        let script = script.unwrap_or_else(|e| panic!("{e}"));
        lua.load(script).exec()?;
    }

    Ok(lua)
}

fn init_global(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // registry
    globals.set("register_node", lua.create_function(register_node)?)?;

    // pins
    globals.set("float", lua.create_function(float)?)?;

    // controls
    globals.set("slider", lua.create_function(slider)?)?;
    globals.set("show_float", lua.create_function(show_float)?)?;

    Ok(())
}

fn register_node(lua: &Lua, args: LuaTable) -> LuaResult<()> {
    let id: String = args.get("id")?;
    let name: String = args.get("name")?;
    let title: String = args.get("title")?;
    let inputs: Option<LuaTable> = args.try_get(lua, "inputs")?;
    let outputs: Option<LuaTable> = args.try_get(lua, "outputs")?;
    let controls: Option<LuaTable> = args.try_get(lua, "controls")?;
    let run: LuaFunction = args.get("run")?;

    let inputs = inputs.map_or_else(|| Ok(IndexMap::default()), |t| t.pairs().collect())?;
    let outputs = outputs.map_or_else(|| Ok(IndexMap::default()), |t| t.pairs().collect())?;
    let controls = controls.map_or_else(|| Ok(IndexMap::default()), |t| t.pairs().collect())?;
    let run = lua.create_registry_value(run)?;

    for key in controls.keys() {
        if inputs.contains_key(key) {
            panic!("control key {} is also an input", key);
        }
        if outputs.contains_key(key) {
            panic!("control key {} is also an output", key);
        }
    }

    let prototype = Arc::new(NodePrototype {
        id: id.clone(),
        title,
        inputs,
        outputs,
        controls,
        run: Box::new(move |lua, inputs| {
            let run: LuaFunction = lua.registry_value(&run)?;
            let result = run.call::<_, LuaTable>(inputs)?;
            Ok(result)
        }),
    });

    REGISTRY.lock().insert(id, prototype.clone());

    let mut category = CATEGORY.lock();
    let mut category = &mut *category;
    let name = name.split("::").collect::<Vec<_>>();
    for name in name.iter().take(name.len() - 1) {
        category = match category
            .entry(name.to_string())
            .or_insert(NodeEntry::Category(BTreeMap::new()))
        {
            NodeEntry::Node(_) => return Err(LuaError::external("category conflict")),
            NodeEntry::Category(dir) => dir,
        }
    }
    category.insert(name.last().unwrap().to_string(), NodeEntry::Node(prototype));

    Ok(())
}

fn float(_lua: &Lua, _args: ()) -> LuaResult<Pin> {
    Ok(Pin::Float)
}

fn slider(_lua: &Lua, args: LuaTable) -> LuaResult<Control> {
    let min: f32 = args.get("min")?;
    let max: f32 = args.get("max")?;
    let value: f32 = args.get("value")?;

    Ok(Control::Slider { min, max, value })
}

fn show_float(_lua: &Lua, args: LuaTable) -> LuaResult<Control> {
    let value: f32 = args.get("value")?;
    Ok(Control::ShowFloat { value })
}

trait LuaTableExt<'a> {
    fn try_get<K, V>(&self, lua: &'a Lua, key: K) -> LuaResult<Option<V>>
    where
        K: IntoLua<'a>,
        V: FromLua<'a>;
}

impl<'a> LuaTableExt<'a> for LuaTable<'a> {
    fn try_get<K, V>(&self, lua: &'a Lua, key: K) -> LuaResult<Option<V>>
    where
        K: IntoLua<'a>,
        V: FromLua<'a>,
    {
        let v: LuaValue = self.get(key)?;
        Ok(if v.is_nil() {
            None
        } else {
            Some(V::from_lua(v, lua)?)
        })
    }
}
