-- The HUD's cameras (beside hud.lua on its entity): a DepthOnly viewmodel camera
-- stacked on the world camera, drawing a gun of boxes on its own layer, and a
-- top-down minimap camera into the render texture "minimap".

local Cams = {
    fields = {
        follow = { type = "string", default = "Player",
            tooltip = "The entity the minimap centres on" },
        camera = { type = "string", default = "Camera",
            tooltip = "The world camera the viewmodel stacks on; made when missing" },
        viewmodel_layer = { type = "number", default = 30, range = { 1, 31 },
            tooltip = "The layer only the viewmodel camera draws" },
    },
}

local MINIMAP_SIZE = 256 -- texels a side
local MINIMAP_SPAN = 40  -- m top to bottom
local MINIMAP_HEIGHT = 60
local KICK = 0.06        -- m back per shot
local KICK_RECOVER = 0.5 -- m a second
local GUN_AT = { 0.22, -0.2, -0.45 }

Cams.kick = 0

local function part(parent, layer, x, y, z, sx, sy, sz, r, g, b)
    local id = Scene.CreateEntity("Gun_Part", "Box")
    Scene.RemoveComponent(id, "Collider")
    Scene.SetParent(id, parent)
    Transform.SetLocalPosition(id, x, y, z)
    Transform.SetLocalScale(id, sx, sy, sz)
    Material.SetColor(id, { r, g, b })
    Layers.SetLayer(id, layer)
    return id
end

local function viewmodel(self)
    local layer = self.viewmodel_layer
    Layers.SetName(layer, "Viewmodel")
    local world = Scene.FindEntityByName(self.camera)
    if world == nil then
        local x, y, z = Camera.GetPosition()
        local yaw, pitch, fov = Camera.GetYaw(), Camera.GetPitch(), Camera.GetFov()
        world = Scene.CreateEntity(self.camera)
        Scene.AddComponent(world, "Camera")
        Camera.SetPosition(x, y, z)
        Camera.SetYaw(yaw)
        Camera.SetPitch(pitch)
        Camera.SetFov(fov)
    end
    Camera.SetCullingMask(world, Camera.GetCullingMask(world) & ~(1 << layer))
    local cam = Scene.CreateEntity("Viewmodel_Camera")
    Scene.AddComponent(cam, "Camera")
    Scene.SetParent(cam, world)
    Transform.SetLocalPosition(cam, 0, 0, 0)
    Camera.SetRenderOrder(cam, 1)
    Camera.SetClearFlags(cam, "DepthOnly")
    Camera.SetCullingMask(cam, 1 << layer)
    Camera.SetFieldOfView(cam, 55)
    Camera.SetClipPlanes(cam, 0.01, 10)
    local gun = Scene.CreateEntity("Gun")
    Scene.SetParent(gun, cam)
    Layers.SetLayer(gun, layer)
    part(gun, layer, 0, 0, 0, 0.06, 0.08, 0.36, 0.15, 0.15, 0.17)
    part(gun, layer, 0, 0.01, -0.3, 0.025, 0.025, 0.3, 0.1, 0.1, 0.1)
    part(gun, layer, 0, -0.09, 0.08, 0.05, 0.12, 0.05, 0.2, 0.13, 0.08)
    part(gun, layer, 0, -0.1, -0.06, 0.04, 0.14, 0.07, 0.12, 0.12, 0.12)
    part(gun, layer, 0, 0.06, 0.02, 0.015, 0.03, 0.08, 0.6, 0.6, 0.6)
    self.gun = gun
end

local function minimap(self)
    local cam = Scene.CreateEntity("Minimap_Camera")
    Scene.AddComponent(cam, "Camera")
    Transform.SetRotation(cam, -90, 0, 0)
    Camera.SetProjection(cam, "Orthographic", MINIMAP_SPAN / 2)
    Camera.SetTargetTexture(cam, "minimap", MINIMAP_SIZE, MINIMAP_SIZE)
    Camera.SetTargetPostFx(cam, false)
    Camera.SetCullingMask(cam, Camera.GetCullingMask(cam) & ~(1 << self.viewmodel_layer))
    self.minimap_cam = cam
end

function Cams.Start(id)
    viewmodel(Cams)
    minimap(Cams)
end

function Cams.Fired()
    Cams.kick = KICK
end

function Cams.LateUpdate(id, dt)
    Cams.kick = math.max(0, Cams.kick - KICK_RECOVER * dt)
    Transform.SetLocalPosition(Cams.gun, GUN_AT[1], GUN_AT[2], GUN_AT[3] + Cams.kick)
    local follow = Scene.FindEntityByName(Cams.follow)
    if follow ~= nil then
        local x, y, z = Transform.GetPosition(follow)
        Transform.SetPosition(Cams.minimap_cam, x, y + MINIMAP_HEIGHT, z)
    end
end

return Cams
