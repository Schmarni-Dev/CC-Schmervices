local util_request = http.get("http://schmerver.mooo.com:63190/files/money_money.lua")
if util_request == nil then return end
local util_code = util_request.readAll()
util_request.close()
if util_code == nil then return end
---@module 'money_money'
local money = assert(loadstring(util_code))()

-- Start
money:set_err_handler(function(...)
    printError(...)
end)
money:set_api_url("http://schmerver.mooo.com:3000")
local token = ""
local f = fs.open("token", "r")
if f == nil then
    local otp = tonumber(io.read())
    ---@diagnostic disable-next-line: param-type-mismatch
    token = money:login("schmarni", otp)
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
local user = money:get_user(token)
print(user:username())
money.schmoneys:insert(money.new_schmoney("Hello World!"))
money:handle_schmoney_list_requests()()
