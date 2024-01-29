local util_request = http.get("http://schmerver.mooo.com:3000/lua/schmervice_lib.lua")
if util_request == nil then
    error("util file request failed")
end
local util_code = util_request.readAll()
util_request.close()
if util_code == nil then return end
---@module 'schmervice_lib'
local schmervice_lib = assert(loadstring(util_code))()
-- local money = require("money_money")

schmervice_lib:set_err_handler(function(...)
    printError(...)
end)


---@type Modem
---@diagnostic disable-next-line: assign-type-mismatch
local modem = peripheral.wrap "back" or error("No Modem at back", 0)
if modem.transmit == nil then
    error "peripheral is not a modem"
end
modem.closeAll()
rednet.open("back")

local range = 16

local schmervice_names = schmervice_lib:get_schmervices_nearby(modem, range)
local function update_schmervices()
    os.sleep(0)
    schmervice_names = schmervice_lib:get_schmervices_nearby(modem, range)
end

local cursor_height = 1

local function event_loop()
    local e, p1, p2 = os.pullEvent()
    if e == "key" then
        local key = keys.getName(p1)
        -- print(key)
        if key == "down" then
            cursor_height = math.min(cursor_height + 1, table.maxn(schmervice_names))
        end
        if key == "up" then
            cursor_height = math.max(cursor_height - 1, 1)
        end
        if key == "enter" and p2 == false then
            local schmervice = schmervice_names[cursor_height]
            if schmervice == nil then
                return
            end
            schmervice_lib:send_join_request_to_schmervice("schmarni", schmervice)
        end
        if key == "q" and p2 == false then
            os.queueEvent("terminate")
        end
    end
end

local function render_loop()
    term.clear()
    term.setCursorPos(1, 1)
    for i, v in ipairs(schmervice_names) do
        local select = " "
        if i == cursor_height then
            select = ">"
        end
        write(select)
        if v.in_use then
            term.setTextColor(colors.red)
        end
        print(v.name)
        term.setTextColor(colors.white)
    end
end

local l = schmervice_lib.loop





parallel.waitForAny(l(render_loop), l(update_schmervices, true), l(event_loop, true))
