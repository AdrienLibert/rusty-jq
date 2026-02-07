import rusty_jq
import json

def test_process():
    data = '{"user_id": 123, "role": "Data Engineer", "city": "Hong Kong"}'
    
    result = rusty_jq.process(".", data)
    print(result)

    result = rusty_jq.process(".role", data)
    print(result)

    result = rusty_jq.process(".salary", data)
    print(result)

    data = '[{"user_id": 123, "role": "Data Engineer"}, {"user_id": 456, "role": "Manager"}]'

    result = rusty_jq.process(".[0]", data)
    print(result)

    result = rusty_jq.process(".[-1]", data)
    print(result)

    result = rusty_jq.process(".[99]", data)
    print(result)

if __name__ == "__main__":
    test_process()
