from transit_model_python import (
    PythonDownloadableModel,
    NTFSDownloader,
    PythonModelConfig,
    PythonNavitiaConfig
)
import asyncio

print("Hello World! from python_test.py")


class CustomDownloader(NTFSDownloader):
    def __new__(cls):
        # Create instance by passing the class as 'obj' to parent __new__
        return super().__new__(cls, cls)
    async def run_download(self, config: PythonModelConfig, version: str) -> str:
        print(f"Downloading model v{version}")
        return "../tests/fixtures/minimal_ntfs/" 


config = PythonModelConfig(
    check_interval_secs=3600,
    path="./models"
)

navitia_config = PythonNavitiaConfig(
    navitia_url="http://localhost:8080",
    navitia_token="dummy_token",
    coverage="test_coverage"
)


async def main():
    model = PythonDownloadableModel(
        navitia_config=navitia_config,
        model_config=config,
        downloader=CustomDownloader()
    )
    
    
    lines = model.get_lines("B42")
    print(lines)  # Should return ["Metro 1"]


if __name__ == "__main__":
    asyncio.run(main())