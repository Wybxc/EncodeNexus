register_node({
    id = "debug.show_float",
    name = "Debug::Show Float",
    title = "Show Float",
    inputs = { x = float() },
    outputs = {},
    controls = {
        number = show_float({ value = 0. })
    },
    run = function (data) 
        return { number = data.x } 
    end,
})
