use std::cell::OnceCell;
use std::sync::Arc;

use egui::Color32;
use egui_snarl::ui::PinInfo;
use indexmap::IndexMap;
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Node {
    prototype: Arc<NodePrototype>,
    pub data: IndexMap<String, Control>,
}

impl Node {
    pub fn title(&self) -> &str {
        &self.prototype.title
    }

    pub fn inputs(&self) -> &IndexMap<String, Pin> {
        &self.prototype.inputs
    }

    pub fn input_name(&self, index: usize) -> &str {
        self.prototype.inputs.get_index(index).unwrap().0
    }

    pub fn outputs(&self) -> &IndexMap<String, Pin> {
        &self.prototype.outputs
    }

    pub fn output_name(&self, index: usize) -> &str {
        self.prototype.outputs.get_index(index).unwrap().0
    }

    pub fn run<'a>(&self, lua: &'a Lua, input: LuaTable<'a>) -> LuaResult<LuaTable<'a>> {
        (self.prototype.run)(lua, input)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromLua)]
pub enum Pin {
    Float,
}

impl mlua::UserData for Pin {}

impl Pin {
    pub fn info(&self) -> PinInfo {
        match self {
            Pin::Float => PinInfo::square().with_fill(Color32::LIGHT_BLUE),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, FromLua)]
pub enum Control {
    Slider { value: f32, min: f32, max: f32 },
    ShowFloat { value: f32 },
}

impl mlua::UserData for Control {}

impl Control {
    pub fn update(&mut self, ui: &mut egui::Ui) {
        match self {
            Control::Slider { value, min, max } => {
                ui.add(egui::Slider::new(value, *min..=*max));
            }
            Control::ShowFloat { value } => {
                ui.label(format!("{:.2}", value));
            }
        }
    }

    pub fn get_value<'a>(&self, lua: &'a mlua::Lua) -> mlua::Result<mlua::Value<'a>> {
        match self {
            Control::Slider { value, .. } => value.into_lua(lua),
            Control::ShowFloat { value } => value.into_lua(lua),
        }
    }

    pub fn set_value(&mut self, lua: &Lua, lua_value: mlua::Value) -> mlua::Result<()> {
        match self {
            Control::Slider { value, .. } => *value = f32::from_lua(lua_value, lua)?,
            Control::ShowFloat { value } => *value = f32::from_lua(lua_value, lua)?,
        }
        Ok(())
    }
}

#[allow(clippy::type_complexity)]
pub struct NodePrototype {
    pub id: String,
    pub title: String,
    pub inputs: IndexMap<String, Pin>,
    pub outputs: IndexMap<String, Pin>,
    pub controls: IndexMap<String, Control>,
    pub run: Box<dyn for<'a> Fn(&'a Lua, LuaTable<'a>) -> LuaResult<LuaTable<'a>> + Send + Sync>,
}

impl std::fmt::Debug for NodePrototype {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("NodePrototype")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("controls", &self.controls)
            .field("run", &"...")
            .finish()
    }
}

impl NodePrototype {
    pub fn unknown(id: String) -> Self {
        NodePrototype {
            id,
            title: "Unknown".to_string(),
            inputs: IndexMap::new(),
            outputs: IndexMap::new(),
            controls: IndexMap::new(),
            run: Box::new(|_, _| Err(LuaError::external("unknown node type"))),
        }
    }

    pub fn create(self: Arc<Self>) -> Node {
        let data = self.controls.clone();
        Node {
            prototype: self,
            data,
        }
    }
}

impl Node {
    pub fn find_factory<T>(
        id: String,
        with_factory: impl FnOnce(Option<Arc<NodePrototype>>) -> T,
    ) -> T {
        let registry = crate::script::REGISTRY.lock();
        with_factory(registry.get(&id).cloned())
    }

    pub fn from_data(id: String, data: IndexMap<String, Control>) -> Node {
        Node::find_factory(id.clone(), |factory| {
            let Some(prototype) = factory else {
                return Node {
                    data,
                    prototype: Arc::new(NodePrototype::unknown(id)),
                };
            };
            // TODO: check that the controls match the factory's controls
            Node { data, prototype }
        })
    }
}

impl Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut data = serializer.serialize_struct("Node", 2)?;
        data.serialize_field("id", &self.prototype.id)?;
        data.serialize_field("controls", &self.data)?;
        data.end()
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Node, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NodeVisitor;
        impl<'de> serde::de::Visitor<'de> for NodeVisitor {
            type Value = Node;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Node")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let id = OnceCell::new();
                let controls = OnceCell::new();
                while let Some(key) = map.next_key()? {
                    match key {
                        "id" => {
                            id.set(map.next_value()?)
                                .map_err(|_| serde::de::Error::duplicate_field("id"))?;
                        }
                        "controls" => {
                            controls
                                .set(map.next_value()?)
                                .map_err(|_| serde::de::Error::duplicate_field("controls"))?;
                        }
                        _ => {}
                    }
                }
                let id = id
                    .into_inner()
                    .ok_or_else(|| serde::de::Error::missing_field("id"))?;
                let controls = controls
                    .into_inner()
                    .ok_or_else(|| serde::de::Error::missing_field("controls"))?;
                Ok(Node::from_data(id, controls))
            }
        }

        deserializer.deserialize_struct("Node", &["id", "controls"], NodeVisitor)
    }
}
