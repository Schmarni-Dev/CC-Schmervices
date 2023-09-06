---@alias AUTH-HEADER "Money-Auth-Key"
---@alias schmoney {name: string, is_used: boolean, username: string, enabled: boolean}

---@return schmoney_list list
local function new_schmoney_list()
    ---@class schmoney_list
    ---@field package [integer] schmoney
    local SL = {}

    ---@param schmoney schmoney
    function SL:insert(schmoney)
        table.insert(self, schmoney)
    end

    ---@param name string
    ---@return schmoney | nil schmoney
    function SL:find(name)
        for _, schmoney in ipairs(self) do
            if schmoney.name == name then
                return schmoney
            end
        end
    end

    ---@param name string
    ---@param enabled boolean
    function SL:set_enabled(name, enabled)
        for _, schmoney in ipairs(self) do
            if schmoney.name == name then
                schmoney.enabled = enabled
            end
        end
    end

    ---This is slow as FUCK Please only use if realy necessary
    ---Please consider using set_active(name,false) for a similar effect and way better performance
    ---@param name string
    ---@return boolean sucsess
    function SL:remove(name)
        local found = false
        local new = new_schmoney_list()
        for _, schmoney in ipairs(self) do
            if schmoney.name == name then
                found = true
            end
        end
        if found then
            for _, schmoney in ipairs(self) do
                if schmoney.name ~= name then
                    new:insert(schmoney)
                end
            end
            self = new
        end
        return found
    end

    function SL:clear()
        self = new_schmoney_list()
    end

    return SL
end
---@class MoneyMoney
---@field package server_url string | nil
---@field package check_url fun(self: MoneyMoney)
---@field package url_checked boolean
---@field package err fun(...)
local M = {}
M.url_checked = false
M.schmoneys = new_schmoney_list()


function M:check_url()
    if self.url_checked then
        return
    end
    if self.server_url == nil then
        self:err("Please set the Server Url")
    end
end

---@param to_user string the username of the buyer
---@param transaction_name string the Title of the Transaction that gets displayed to the user
---@param transaction_amount integer the amount of money of this transaction
---@param user user the user that acts as the seller in this transaction
---@param ... any these will be passed into the event emitted as args
---@return function awaitable please run this function using the paralel or just blocking, ig
function M:make_transaction(to_user, transaction_name, transaction_amount, user, ...)
    local request_data = { buyer = to_user, amount = transaction_amount, name = transaction_name }
    local resp = self:make_authed_api_request("/api/request_transaction", user, request_data)

    local headers = { ["Money-Auth-Key"] = user:token() }
    local socket, err = http.websocket("/api/notify_transaction/" .. resp[1])

    if socket == false then
        self.err(err)
        return function()

        end
    end
    local args = ...
    return function()
        while true do
            local msg = socket.receive()
            if msg == "transaction_rejected" then break end
            if msg == "transaction_accepted" then
                os.queueEvent("money:on_transaction_complete", args)
            end
        end
    end
end

---@param endpoint string the endpoint to hit include the begining  /
---@param data table the data to send to the endpoint
---@param headers? table<HTTP_REQUEST_HEADERS | AUTH-HEADER>
---@return table data
---@return string | nil err
function M:make_api_request(endpoint, data, headers)
    self:check_url()
    local encoded_data = textutils.serializeJSON(data)
    local url = self.server_url .. endpoint
    if headers == nil then
        headers = {}
    end
    headers.Accept = "application/json"
    headers["Content-Type"] = "application/json"
    print(encoded_data)
    local response, err = http.post(url, encoded_data, headers)
    if response == nil then
        return {}, err
    end
    local response_text = response.readAll()
    if response_text == nil then
        return {}, "Unable to Decode Text from Response"
    end
    local value = textutils.unserializeJSON(response_text, { parse_empty_array = true })
    if value == nil then
        return {}, "Unable to Desirealize Response: " .. response_text
    end
    if type(value) ~= "table" then
        value = { value }
    end
    return value, nil
end

---@param endpoint string the endpoint to hit include the begining  /
---@param user user The user the Request is Send as
---@param data table the data to send to the endpoint
---@param headers? table<HTTP_REQUEST_HEADERS>
---@return table data
---@return string | nil err
function M:make_authed_api_request(endpoint, user, data, headers)
    headers = headers or {}
    headers["Money-Auth-Key"] = user:token()
    return self:make_api_request(endpoint, data, headers)
end

---@param token string Valid AuthToken
---@return user
---@nodiscard
function M:get_user(token)
    local money = self
    ---@class user
    ---@field private auth_token string
    ---@field private name string | nil
    ---@field private server_url string
    local U = { auth_token = token }
    ---@return string username The users display name
    function U:username()
        if self.name ~= nil then
            return self.name
        end
        local name, err = money:make_api_request("/api/get_displayname", { request_token = self:token() })
        if err ~= nil then
            money:err("error getting User Display Name:", err)
        end
        self.name = name[1]

        return name[1]
    end

    ---@return string username The users display name
    function U:token()
        return self.auth_token
    end

    return U
end

---@param username string
---@param otp integer
---@return string token
---@nodiscard
function M:login(username, otp)
    local value, err = self:make_api_request("/login", { username = username, otp = otp })
    if err ~= nil then
        self:err("Unable to login", err)
    end
    print("TEST:")
    textutils.tabulate(value)
    print(":TEST")
    local token = value["auth_token"]
    if type(token) ~= "string" then
        self:err("Not a Valid Login Response")
    end
    return token
end

---@param timeout number
---@return function timeout
function M.timeout(timeout)
    local timer = os.startTimer(timeout)
    return function()
        while true do
            local _, t = os.pullEvent("timer")
            if t == timer then
                break
            end
        end
    end
end

---@return {name:string,rednet_id:number}[]
function M:get_schmoneys_nearby()
    rednet.broadcast("", "SCHMONEY_LIST")

    local messages = {}
    local function handle_msgs()
        while true do
            local sender, name = rednet.receive("SCHMONEY_LIST_RESPONSE")
            table.insert(messages, { name = name, rednet_id = sender })
        end
    end

    parallel.waitForAny(handle_msgs, self.timeout(0.1))
    return messages
end

function M:handle_schmoney_list_requests()
    return function()
        while true do
            local sender = rednet.receive("SCHMONEY_LIST")
            if sender ~= nil then
                for _, v in ipairs(self.schmoneys) do
                    rednet.send(sender, v.name, "SCHMONEY_LIST_RESPONSE")
                end
            end
        end
    end
end

---@param name string
---@param enabled? boolean
---@return schmoney
function M.new_schmoney(name, enabled)
    if enabled == nil then
        enabled = true
    end
    return { name = name, username = "", is_used = false, enabled = enabled }
end

---@param handler fun(...)
---@return fun(...) | nil old_handler
function M:set_err_handler(handler)
    local old = self.err
    self.err = handler
    return old
end

---@param url string
---@return string | nil old_url
function M:set_api_url(url)
    local old = self.server_url
    self.server_url = url
    return old
end

return M
