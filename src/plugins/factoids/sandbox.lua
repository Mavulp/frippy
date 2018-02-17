function send(text)
  local text = tostring(text)
  local len = #output
  if len < 1 then
    output = { text }
  else
    output[len] = output[len] .. text
  end
end

function sendln(text)
  send(text)
  table.insert(output, "")
end

local env = { print = send,
              println = sendln,
              args = args,
              input = input,
              user = user,
              channel = channel,
              request = download,
              pairs = pairs,
              table = table,
              string = string,
              tostring = tostring,
              math = math }

local f, e = load(factoid, nil, nil, env)
if f then
  f()
else
  error(e)
end
