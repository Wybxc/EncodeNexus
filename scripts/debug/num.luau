register_node({
    id = "debug.num",
    name = "Debug::Number",
    title = "Number",
    inputs = {},
    outputs = { x = float() },
    controls = {
        number = slider({ min = 0., max = 100., value = 50. })
    },
    run = function (data) 
        return { x = data.number } 
    end,
})
