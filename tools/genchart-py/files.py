import glob
import os
import json
from typing import List, Dict
from processFiles import CommitData

DATA_DIRECTORY = "data"


def read_json_file(file_path: str) -> Dict:
    with open(file_path, "r") as file:
        try:
            return json.load(file)
        except (json.JSONDecodeError, KeyError) as e:
            print(f"Error parsing {file_path}: {e}")
    return {}


def process_files() -> CommitData:
    data = CommitData()
    for file_path in glob.glob(os.path.join(DATA_DIRECTORY, "raw_*.json")):
        json_data = read_json_file(file_path)
        if json_data:
            data.process_json_data(json_data)
    return data


def write_to_js_file(variables: Dict[str, List]) -> None:
    with open("chart_data.js", "w") as js_file:
        for var_name, value in variables.items():
            js_file.write(f"const {var_name} = {json.dumps(value)};\n")
