function send(text)
  len = #output
  if len < 1 then
    output = { text }
  else
    output[len] = output[len] .. text
  end
end

function sendln(text)
  sendtoirc(text)
  table.insert(output, "")
end

local env = { print = send,
              println = sendln,
              args = args,
              input = input,
              user = user,
              channel = channel,
              pairs = pairs,
              table = table,
              string = string,
              tostring = tostring,
              math = math }

load(factoid, nil, nil, env)()
