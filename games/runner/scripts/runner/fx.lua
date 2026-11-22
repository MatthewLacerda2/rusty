
local Fx = {}

local CAMERA_BACK, CAMERA_UP = 6.5, 3.2 -- m behind and above the runner
local CAMERA_PITCH = -15                -- degrees, looking down at the track
local CAMERA_FOLLOW_X = 0.6             -- how much of a lane change the camera follows

function Fx.new()
    local hud = Scene.FindEntityByName("HUD")
    return {
        dust = Scene.FindEntityByName("Dust"),
        sparks = Scene.FindEntityByName("Sparks"),
        distance = Scene.FindChild(hud, "Distance"),
        over = Scene.FindChild(hud, "GameOver"),
        shown = -1,
    }
end

function Fx.dust(fx, x, y, z, count)
    Transform.SetPosition(fx.dust, x, y, z)
    Particles.Emit(fx.dust, count)
end

function Fx.sparks(fx, x, y, z)
    Transform.SetPosition(fx.sparks, x, y, z)
    Particles.Emit(fx.sparks, 24)
end

function Fx.sound(name, x, y, z, volume)
    Audio.PlayAt("assets/sounds/" .. name .. ".wav", x, y, z, volume or 0.8)
end

-- The camera follows only some of a lane change, so the runner stays near the centre.
function Fx.follow(x, z)
    Camera.SetPosition(x * CAMERA_FOLLOW_X, CAMERA_UP, z - CAMERA_BACK)
    Camera.SetYaw(90) -- down +Z
    Camera.SetPitch(CAMERA_PITCH)
end

function Fx.hud(fx, metres, over)
    local whole = math.floor(metres)
    if whole ~= fx.shown then
        Text.SetText(fx.distance, whole .. " m")
        fx.shown = whole
    end
    if Scene.IsActiveInHierarchy(fx.over) ~= over then Scene.SetActive(fx.over, over) end
end

return Fx
