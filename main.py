from tryx_store_postgres import PostgresStore, detect_platform


def main():
    os_family, arch, variant = detect_platform()
    print(f"Platform: {os_family}-{arch}" + (f"-{variant}" if variant else ""))

    store = PostgresStore(
        host="localhost",
        port=5432,
        database="tryx",
        user="postgres",
        password="",
    )
    print(f"Library path: {store.lib_path}")
    print(f"Config: {store.config_json}")


if __name__ == "__main__":
    main()
