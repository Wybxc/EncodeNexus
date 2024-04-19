local t = i18n_translate

register_node({
    id = "debug.add",
    name = "Debug::Add",
    title = "Add",
    inputs = { a = float(), b = float() },
    outputs = { result = float() },
    run = function (data) 
        return { result = data.a + data.b } 
    end,
})