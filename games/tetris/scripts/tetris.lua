-- tetris.lua — the falling-blocks game: input, the fall, locking, line clears and
-- game over, on the fixed tick. The grid rules are `tetris/rules.lua`; the cubes,
-- the burst, the light, the sounds and the HUD are `tetris/view.lua`.
--
-- Keys: Left/Right move (held, they repeat after a short delay), Up or X turns
-- clockwise, Z counter-clockwise, Down drops softly, Space drops hard, Enter
-- starts over after game over.
--
-- A scenario reads the state through `Scene.GetScript(id, "tetris")`: `board`
-- (`board[row][col]`, row 1 at the bottom, 0 empty), `score`, `lines`, `level`,
-- `over`, `kind` / `next` (piece kinds, `rules.NAMES`) and `pieces` (how many
-- have spawned). It may write `level`, and call `game:Restart()`,
-- `game:Stage(rows, kind)` and `game:CubeAt(row, col)`.

local Rules = require("tetris.rules")
local View = require("tetris.view")

local Tetris = {
    fields = {
        width = { type = "number", default = 10, range = { 4, 30 }, tooltip = "Columns" },
        height = { type = "number", default = 20, range = { 4, 40 }, tooltip = "Visible rows" },
        start_level = { type = "number", default = 1, range = { 1, 20 }, tooltip = "Level at the start" },
        seed = { type = "number", default = 1, tooltip = "Seeds the piece order" },
    },
}

local DAS_TICKS = 10      -- how long Left/Right is held before it repeats
local ARR_TICKS = 3       -- ticks between repeats once it does
local SOFT_TICKS = 2      -- ticks per row while Down is held
local HIDDEN_ROWS = 4     -- rows above the well a piece spawns into

local piece = {}          -- the falling piece: kind, rot, col, row, ids
local cells = {}          -- cells[row][col]: the cube entity of a locked cell
local bag = {}            -- the kinds still to come in this bag
local bursts = {}         -- cubes flying out of a cleared line
local hud, light
local pulse = 0
local fall_wait, held, held_for = 0, nil, 0

-- The next kind from the bag, refilled with a fresh shuffle when it runs out.
local function draw_kind()
    if #bag == 0 then
        bag = Rules.bag(function(n) return Random.Range(1, n + 1) end)
    end
    return table.remove(bag, 1)
end

local function rows_total() return Tetris.height + HIDDEN_ROWS end

local function place_piece()
    for i, c in ipairs(Rules.cells(piece.kind, piece.rot)) do
        View.place(Tetris.width, piece.ids[i], piece.row + c[2], piece.col + c[1])
    end
end

-- Spawn `kind` (or the next kind) at the top; a spawn that collides ends the game.
local function spawn(kind)
    piece.kind = kind or Tetris.next
    Tetris.next = draw_kind()
    piece.rot = 0
    piece.col, piece.row = Rules.spawn(piece.kind, Tetris.width, Tetris.height + 2)
    piece.ids = {}
    for i = 1, 4 do piece.ids[i] = View.cube(Tetris.width, piece.kind, 1, 1) end
    place_piece()
    Tetris.kind = piece.kind
    Tetris.pieces = Tetris.pieces + 1
    fall_wait = Rules.fall_ticks(Tetris.level)
    View.show_next(hud, Rules.cells(Tetris.next, 0), Tetris.next)
    if not Rules.fits(Tetris.board, piece.kind, piece.rot, piece.col, piece.row) then
        Tetris.over = true
        View.sound("over")
    end
end

-- Clear the full rows: their cubes burst, the cubes above drop to fill the gap.
local function clear_lines()
    local full = Rules.full_rows(Tetris.board)
    if #full == 0 then return 0 end
    for _, r in ipairs(full) do
        for c = 1, Tetris.width do View.burst(bursts, Tetris.width, cells[r][c], c) end
    end
    Rules.collapse(Tetris.board, full)
    Rules.collapse(cells, full)
    for r = 1, rows_total() do
        for c = 1, Tetris.width do
            if cells[r][c] ~= 0 then View.place(Tetris.width, cells[r][c], r, c) end
        end
    end
    Tetris.lines = Tetris.lines + #full
    Tetris.score = Tetris.score + Rules.line_points(#full, Tetris.level)
    Tetris.level = math.max(Tetris.level, Rules.level(Tetris.start_level, Tetris.lines))
    View.sound("clear")
    return #full
end

-- Lock the falling piece into the board, clear what it filled, spawn the next.
local function lock()
    local placed = Rules.lock(Tetris.board, piece.kind, piece.rot, piece.col, piece.row)
    for i, rc in ipairs(placed) do cells[rc[1]][rc[2]] = piece.ids[i] end
    View.sound("lock")
    local cleared = clear_lines()
    if Rules.topped_out(Tetris.board, Tetris.height) then
        Tetris.over = true
        View.sound("over")
        return cleared
    end
    spawn()
    return cleared
end

-- Move the piece by (`dc`, `dr`) if it fits there.
local function shift(dc, dr)
    if not Rules.fits(Tetris.board, piece.kind, piece.rot, piece.col + dc, piece.row + dr) then
        return false
    end
    piece.col, piece.row = piece.col + dc, piece.row + dr
    place_piece()
    return true
end

local function turn(dir)
    local rot, col = Rules.rotate(Tetris.board, piece.kind, piece.rot, piece.col, piece.row, dir)
    if rot == nil then return end
    piece.rot, piece.col = rot, col
    place_piece()
    View.sound("rotate")
end

-- Left/Right: one step on the press, then a repeat every ARR_TICKS after DAS_TICKS.
local function sideways()
    local dir = (Input.IsKeyDown("LEFT") and -1 or 0) + (Input.IsKeyDown("RIGHT") and 1 or 0)
    if Input.GetKeyDown("LEFT") then dir, held, held_for = -1, -1, 0; shift(-1, 0); return end
    if Input.GetKeyDown("RIGHT") then dir, held, held_for = 1, 1, 0; shift(1, 0); return end
    if dir == 0 or dir ~= held then held = nil; return end
    held_for = held_for + 1
    if held_for >= DAS_TICKS and (held_for - DAS_TICKS) % ARR_TICKS == 0 then shift(dir, 0) end
end

-- One tick of play: input, then the fall. Returns the lines cleared this tick.
local function play()
    sideways()
    if Input.GetKeyDown("UP") or Input.GetKeyDown("X") then turn(1) end
    if Input.GetKeyDown("Z") then turn(-1) end
    if Input.GetKeyDown("SPACE") then
        local to = Rules.drop_row(Tetris.board, piece.kind, piece.rot, piece.col, piece.row)
        Tetris.score = Tetris.score + 2 * (piece.row - to)
        piece.row = to
        place_piece()
        return lock()
    end
    local soft = Input.IsKeyDown("DOWN")
    fall_wait = fall_wait - 1
    if soft then fall_wait = math.min(fall_wait, SOFT_TICKS) end
    if fall_wait > 0 then return 0 end
    fall_wait = Rules.fall_ticks(Tetris.level)
    if shift(0, -1) then
        if soft then Tetris.score = Tetris.score + 1 end
        return 0
    end
    return lock()
end

-- Destroy every cube, empty the board and start a new game from `seed`.
function Tetris.Restart()
    for _, id in ipairs(piece.ids or {}) do Scene.DestroyEntity(id) end
    for r = 1, #cells do
        for c = 1, #cells[r] do
            if cells[r][c] ~= 0 then Scene.DestroyEntity(cells[r][c]) end
        end
    end
    View.clear_bursts(bursts)
    Random.SetSeed(Tetris.seed)
    Tetris.board = Rules.new_board(Tetris.width, rows_total())
    cells = Rules.new_board(Tetris.width, rows_total())
    bag = {}
    Tetris.score, Tetris.lines, Tetris.pieces = 0, 0, 0
    Tetris.level, Tetris.over = Tetris.start_level, false
    held = nil
    Tetris.next = draw_kind()
    spawn()
end

-- Put a test position on the board: `rows` are strings bottom first, `X` a filled
-- cell and `.` an empty one; the falling piece becomes `kind` (a name) at the top.
function Tetris.Stage(_, rows, kind)
    Tetris.Restart()
    for r, line in ipairs(rows) do
        for c = 1, Tetris.width do
            if line:sub(c, c) == "X" then
                Tetris.board[r][c] = Rules.GARBAGE
                cells[r][c] = View.cube(Tetris.width, Rules.GARBAGE, r, c)
            end
        end
    end
    for _, id in ipairs(piece.ids) do Scene.DestroyEntity(id) end
    Tetris.pieces = Tetris.pieces - 1
    spawn(Rules.kind(kind))
end

-- The cube entity drawing locked cell (`row`, `col`), or nil when it is empty.
function Tetris.CubeAt(_, row, col)
    local id = cells[row] and cells[row][col]
    if id == 0 then return nil end
    return id
end

function Tetris.Start(id)
    -- Inspector fields arrive as floats; the grid counts in whole cells.
    for _, f in ipairs({ "width", "height", "start_level", "seed" }) do
        Tetris[f] = math.floor(Tetris[f])
    end
    hud = View.hud()
    light = Scene.FindEntityByName("Pulse")
    Tetris.Restart()
end

function Tetris.Update(id, dt)
    View.tick_bursts(bursts)
    local cleared = 0
    if Tetris.over then
        if Input.GetKeyDown("ENTER") then Tetris.Restart() end
    else
        cleared = play()
    end
    pulse = View.pulse(light, pulse, cleared)
    View.show_stats(hud, Tetris.score, Tetris.level, Tetris.lines, Tetris.over)
end

return Tetris
