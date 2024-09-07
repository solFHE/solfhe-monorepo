import json
import webbrowser  


def read_json_file(file_path):
    with open(file_path, "r") as file:
        return json.load(file)


blink_links = {
    "superteam": "https://dial.to/developer?url=http://localhost:3001/api/action&cluster=mainnet",  # Superteam linki
    "zk-Lokomotive": "https://dial.to/developer?url=http://localhost:3000/api/action&cluster=mainnet"  # zk-Lokomotive linki
}


def check_for_blink(data):
    for token in data.get("tokens", []):
        if token in blink_links:
            blink_link = blink_links[token]
            print(f"{token} aktif: {blink_link}")
            webbrowser.open(blink_link)  
            return blink_link  
    return "No blink found"


json_file_path = "script.json"


json_data = read_json_file(json_file_path)


blink_status = check_for_blink(json_data)

print(f"Gösterilecek blink: {blink_status}")
