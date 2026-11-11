-- tetris/rules.lua — the falling-blocks rules over a plain grid, with no entity in
-- sight: the seven pieces, rotation, collision, line clears, scoring and the fall
-- speed. `tetris.lua` owns the state and the entities; everything here is a pure
-- function of its arguments, so the same inputs always give the same board.
--
-- The board is `board[row][col]`, row 1 at the bottom, `0` for an empty cell and a
-- piece kind (`1..7`, or `GARBAGE` for a staged cell) for a filled one.

local Rules = {}

Rules.GARBAGE = 8

-- The seven pieces, in their spawn orientation, as cells of an N×N box with y up
-- (`{x, y}`, 0-based). Names index `Rules.KINDS`.
Rules.NAMES = { "I", "O", "T", "S", "Z", "J", "L" }
local SHAPES = {
    { n = 4, cells = { { 0, 2 }, { 1, 2 }, { 2, 2 }, { 3, 2 } } }, -- I
    { n = 2, cells = { { 0, 0 }, { 1, 0 }, { 0, 1 }, { 1, 1 } } }, -- O
    { n = 3, cells = { { 0, 1 }, { 1, 1 }, { 2, 1 }, { 1, 2 } } }, -- T
    { n = 3, cells = { { 0, 1 }, { 1, 1 }, { 1, 2 }, { 2, 2 } } }, -- S
    { n = 3, cells = { { 0, 2 }, { 1, 2 }, { 1, 1 }, { 2, 1 } } }, -- Z
    { n = 3, cells = { { 0, 2 }, { 0, 1 }, { 1, 1 }, { 2, 1 } } }, -- J
    { n = 3, cells = { { 2, 2 }, { 0, 1 }, { 1, 1 }, { 2, 1 } } }, -- L
}

-- Points for 1..4 lines cleared at once, times the level.
local LINE_POINTS = { 100, 300, 500, 800 }

-- The kind named `name` ("I", "t", …), or nil.
function Rules.kind(name)
    for k, n in ipairs(Rules.NAMES) do
        if n == string.upper(name) then return k end
    end
    return nil
end

-- The box size of `kind`'s shape.
function Rules.size(kind)
    return SHAPES[kind].n
end

-- `kind`'s cells at rotation `rot` (0..3 clockwise turns), as box offsets.
function Rules.cells(kind, rot)
    local shape = SHAPES[kind]
    local out = {}
    for i, c in ipairs(shape.cells) do
        local x, y = c[1], c[2]
        for _ = 1, rot % 4 do
            x, y = y, shape.n - 1 - x -- a clockwise quarter turn, y up
        end
        out[i] = { x, y }
    end
    return out
end

-- An empty board `width` wide and `rows` tall.
function Rules.new_board(width, rows)
    local board = {}
    for r = 1, rows do
        board[r] = {}
        for c = 1, width do board[r][c] = 0 end
    end
    return board
end

-- Whether the piece fits: its box's bottom-left cell at (`col`, `row`).
function Rules.fits(board, kind, rot, col, row)
    for _, c in ipairs(Rules.cells(kind, rot)) do
        local r, k = row + c[2], col + c[1]
        local line = board[r]
        if line == nil or line[k] == nil or line[k] ~= 0 then return false end
    end
    return true
end

-- The column and row a new `kind` spawns at: centred, its top on row `top`.
function Rules.spawn(kind, width, top)
    local n = SHAPES[kind].n
    return math.floor((width - n) / 2) + 1, top - n + 1
end

-- Try to turn the piece by `dir` (1 clockwise, -1 counter-clockwise), kicking it
-- sideways when the turn alone collides. The new rotation and column, or nil.
local KICKS = { 0, -1, 1, -2, 2 }
function Rules.rotate(board, kind, rot, col, row, dir)
    local turned = (rot + dir) % 4
    for _, dx in ipairs(KICKS) do
        if Rules.fits(board, kind, turned, col + dx, row) then return turned, col + dx end
    end
    return nil
end

-- The lowest row the piece reaches falling straight down from `row`.
function Rules.drop_row(board, kind, rot, col, row)
    while Rules.fits(board, kind, rot, col, row - 1) do row = row - 1 end
    return row
end

-- Write the piece into the board. Returns its cells as `{row, col}` pairs.
function Rules.lock(board, kind, rot, col, row)
    local placed = {}
    for i, c in ipairs(Rules.cells(kind, rot)) do
        local r, k = row + c[2], col + c[1]
        board[r][k] = kind
        placed[i] = { r, k }
    end
    return placed
end

-- The full rows, bottom first.
function Rules.full_rows(board)
    local full = {}
    for r, line in ipairs(board) do
        local all = true
        for _, v in ipairs(line) do
            if v == 0 then all = false; break end
        end
        if all then full[#full + 1] = r end
    end
    return full
end

-- Remove `rows` (bottom first) from the board, dropping everything above them.
function Rules.collapse(board, rows)
    for i = #rows, 1, -1 do
        local width = #board[rows[i]]
        table.remove(board, rows[i])
        local line = {}
        for c = 1, width do line[c] = 0 end
        board[#board + 1] = line
    end
end

-- The points for clearing `n` lines at once on `level`.
function Rules.line_points(n, level)
    return (LINE_POINTS[math.min(n, 4)] or 0) * level
end

-- Ticks a piece waits between falling one row on `level` (1 is the slowest).
function Rules.fall_ticks(level)
    return math.max(1, 32 - (level - 1) * 3)
end

-- The level reached from `start` after `lines` lines: one more every ten.
function Rules.level(start, lines)
    return start + lines // 10
end

-- A fresh bag: the seven kinds in an order drawn from `draw(n)` (an integer in
-- `[1, n]`), so every kind comes once per seven pieces.
function Rules.bag(draw)
    local bag = { 1, 2, 3, 4, 5, 6, 7 }
    for i = 7, 2, -1 do
        local j = draw(i)
        bag[i], bag[j] = bag[j], bag[i]
    end
    return bag
end

-- Whether any cell of rows above `visible` is filled: the stack topped out.
function Rules.topped_out(board, visible)
    for r = visible + 1, #board do
        for _, v in ipairs(board[r]) do
            if v ~= 0 then return true end
        end
    end
    return false
end

return Rules
