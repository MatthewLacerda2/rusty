-- The city's cameras are the bench's viewpoints, all active in the saved scene
-- (`make bench-scenes` draws from each in turn). Play draws from one: this keeps
-- the first child camera, the street-level one, and turns the rest off.

local Viewpoints = {}

function Viewpoints.Start(id)
    for n, camera in ipairs(Scene.GetChildren(id)) do
        if n > 1 then Scene.SetActive(camera, false) end
    end
end

return Viewpoints
