from transit_model_python import PythonTransitModel

print("Hello World! from python_test.py")
model = PythonTransitModel("../tests/fixtures/minimal_ntfs/")
lines = model.get_lines("B42")
print(lines)  # Output: ["Metro 1"]