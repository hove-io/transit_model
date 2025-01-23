from transit_model_python import PythonTransitModel

model = PythonTransitModel("../tests/fixtures/minimal_ntfs/")
lines = model.get_lines("M1")
print(lines)  # Output: ["Metro 1"]