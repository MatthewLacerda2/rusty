-- Pure functions of their arguments, so the same inputs always give the same
-- board. `board[row][col]`: row 1 at the bottom, `0` empty, else a piece kind
-- (`1..7`, or `GARBAGE` for a staged cell).

local Rules = {}

Rules.GARBAGE = 8

-- The seven pieces in spawn orientation, as cells of an N×N box with y up (`{x, y}`, 0-based).
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

local LINE_POINTS = { 100, 300, 500, 800 }

function Rules.kind(name)
    for k, n in ipairs(Rules.NAMES) do
        if n == string.upper(name) then return k end
    end
    return nil
end

function Rules.size(kind)
    return SHAPES[kind].n
end

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

function Rules.new_board(width, rows)
    local board = {}
    for r = 1, rows do
        board[r] = {}
        for c = 1, width do board[r][c] = 0 end
    end
    return board
end

function Rules.fits(board, kind, rot, col, row)
    for _, c in ipairs(Rules.cells(kind, rot)) do
        local r, k = row + c[2], col + c[1]
        local line = board[r]
        if line == nil or line[k] == nil or line[k] ~= 0 then return false end
    end
    return true
end

function Rules.spawn(kind, width, top)
    local n = SHAPES[kind].n
    return math.floor((width - n) / 2) + 1, top - n + 1
end

-- Turn by `dir` (1 clockwise, -1 counter-clockwise), kicking sideways when the
-- turn alone collides. Returns the new rotation and column, or nil.
local KICKS = { 0, -1, 1, -2, 2 }
function Rules.rotate(board, kind, rot, col, row, dir)
    local turned = (rot + dir) % 4
    for _, dx in ipairs(KICKS) do
        if Rules.fits(board, kind, turned, col + dx, row) then return turned, col + dx end
    end
    return nil
end

function Rules.drop_row(board, kind, rot, col, row)
    while Rules.fits(board, kind, rot, col, row - 1) do row = row - 1 end
    return row
end

function Rules.lock(board, kind, rot, col, row)
    local placed = {}
    for i, c in ipairs(Rules.cells(kind, rot)) do
        local r, k = row + c[2], col + c[1]
        board[r][k] = kind
        placed[i] = { r, k }
    end
    return placed
end

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

function Rules.collapse(board, rows)
    for i = #rows, 1, -1 do
        local width = #board[rows[i]]
        table.remove(board, rows[i])
        local line = {}
        for c = 1, width do line[c] = 0 end
        board[#board + 1] = line
    end
end

function Rules.line_points(n, level)
    return (LINE_POINTS[math.min(n, 4)] or 0) * level
end

function Rules.fall_ticks(level)
    return math.max(1, 32 - (level - 1) * 3)
end

function Rules.level(start, lines)
    return start + lines // 10
end

-- `draw(n)` returns an integer in [1, n].
function Rules.bag(draw)
    local bag = { 1, 2, 3, 4, 5, 6, 7 }
    for i = 7, 2, -1 do
        local j = draw(i)
        bag[i], bag[j] = bag[j], bag[i]
    end
    return bag
end

function Rules.topped_out(board, visible)
    for r = visible + 1, #board do
        for _, v in ipairs(board[r]) do
            if v ~= 0 then return true end
        end
    end
    return false
end

return Rules
