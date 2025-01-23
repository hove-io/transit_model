# PythonTransitModel

**PythonTransitModel** is a Rust library that provides a Python interface to interact with transit data models. This library allows users to retrieve transit-related data, such as lines, contributors, networks, stop areas, and vehicle journeys, via Python.

## Features

- Retrieve lines passing through a stop.
- Get contributors providing data for a specific stop.
- Retrieve networks using a stop.
- Access stop area details by ID.
- Fetch vehicle journey information by ID, etc.

## Requirements

- Rust
- Python >= 3.8
- [pyo3](https://pyo3.rs/)
- [transit_model](https://github.com/hove-io/transit_model)

## Installation

1. Clone the repository:
   ```bash
   git clone <repository_url>
   cd <repository_directory/transit_model_python>
   python3 -m venv .venv
   source .venv/bin/activate
   ```

2. Install Maturin and Build the Python module:
   ```bash
   pip install maturin
   maturin develop
   ```
   This command builds and installs the Python module locally.

3. Import the module in your Python scripts:
   ```python
   from transit_model_python import PythonTransitModel
   ```

## Usage

### Initializing the Model
```python
model = PythonTransitModel("../tests/fixtures/minimal_ntfs/")
```

### Fetching Lines Passing Through a Stop
```python
lines = model.get_lines("M1")
print(lines)  # Output: ["Metro 1"]
```

### Fetching Contributors Providing Data for a Stop
```python
contributors = model.get_contributors("TGC")
print(contributors)  # Output: ["The Great Contributor"]
```

### Fetching Networks Using a Stop
```python
networks = model.get_networks("TGN")
print(networks)  # Output: ["The Great Network"]
```

### Fetching Stop Area Details by ID
```python
stop_area = model.get_stop_area_by_id("GDL")
print(stop_area)  # Output: "Gare de Lyon"
```

### Fetching Vehicle Journey Details by ID
```python
vehicle_journey = model.get_vehicule_journey_by_id("VJ1")
print(vehicle_journey)  # Output: Optional["Journey Pattern ID"]
```

## Development

### Running Tests
To run the tests, execute:
```bash
cargo test
```

## Contributing
Contributions are welcome! Please follow these steps:

1. Fork the repository.
2. Create a new branch for your feature or bug fix.
3. Submit a pull request.

## License
This project is licensed under the [GNU](LICENSE).

## Acknowledgments
- [pyo3](https://pyo3.rs/) for enabling seamless integration between Rust and Python.
- [transit_model](https://github.com/hove-io/transit_model) for the underlying transit model library.
