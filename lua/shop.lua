local util_request = http.get("http://schmerver.mooo.com:3000/lua/schmervice_lib.lua")
if util_request == nil then error("util file request failed") end
local util_code = util_request.readAll()
util_request.close()
if util_code == nil then return end
---@module 'schmervice_lib'
local schmervice_lib = assert(loadstring(util_code))()



-- Start

---@type Modem
---@diagnostic disable-next-line: assign-type-mismatch
local modem = peripheral.wrap "left" or error("No Modem at left", 0)
if modem.transmit == nil then
    error "peripheral is not a modem"
end
modem.closeAll()


schmervice_lib:set_err_handler(function(...)
    printError(...)
end)

schmervice_lib:set_api_url("http://schmerver.mooo.com:3000")

local token = ""
local f = fs.open("token", "r")
if f == nil then
    local otp = tonumber(io.read())
    ---@diagnostic disable-next-line: param-type-mismatch
    token = schmervice_lib:login("schmarni", otp)
    print(token)
    ---@type WriteHandle
    ---@diagnostic disable-next-line: assign-type-mismatch
    local fw = fs.open("token", "w")
    fw.write(token)
    fw.close()
else
    local w = f.readAll()
    if w ~= nil then
        token = w
    end
end
rednet.open("left")
local user = schmervice_lib:get_user(token)
print(user:username())
schmervice_lib.schmervices:insert(schmervice_lib.new_schmervice("Hello World!"))
schmervice_lib.schmervices:insert(schmervice_lib.new_schmervice("wow how cool"))
schmervice_lib.schmervices:insert(schmervice_lib.new_schmervice("nice chicken"))
local w = schmervice_lib.new_schmervice("i am bored")
w.in_use = true
schmervice_lib.schmervices:insert(w)

parallel.waitForAny(schmervice_lib:handle_schmervice_list_requests(modem), schmervice_lib:handle_schmervice_join_requests())
