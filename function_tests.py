import argparse
import asyncio
import random
import sys
from google.cloud import bigquery
import json
import os
import subprocess

from concurrent.futures import ProcessPoolExecutor


def export_from_bigquery():
    client = bigquery.Client()

    query = """
        SELECT jsonPayload
        FROM `polybase-testnet.testnet_logs.stdout`
        TABLESAMPLE SYSTEM (10 PERCENT)
        WHERE jsonPayload.message = "function output"
        ORDER BY rand()
        LIMIT 10000
    """
    query_job = client.query(query)

    filename = "function_test.jsonl"

    with open(filename, "w") as file:
        for row in query_job.result():
            j = json.dumps(row.jsonPayload)
            file.write(j + '\n')

    print(f"Data exported to {filename}")


async def run_commands(semaphore, collection_code, collection_id, collection_name, function_name, this_instance_json, args, auth, output_result, i, total_lines):
    async with semaphore:
        compile_args = [
            "./target/release/compile",
            "collection:" + collection_name,
            "function:" + function_name,
        ]

        compile_process = await asyncio.create_subprocess_exec(
            *compile_args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        compile_result, compile_error = await compile_process.communicate(input=collection_code.encode())

        if compile_process.returncode != 0:
            print(
                f"Collection: {collection_id}, Function: {function_name} with code:\n{collection_code}\nfailed to compile:")
            print(f"Compile error: {compile_error.decode()}")
            return False

        miden_code = compile_result.decode()

        abi = None
        for line in compile_error.decode().split("\n"):
            if line.startswith("ABI: "):
                abi = line.split("ABI: ")[1]

        run_args = [
            "./target/release/miden-run",
            "--abi",
            abi,
            "--this-json",
            this_instance_json,
            "--advice-tape-json",
            args,
            "--ctx",
            json.dumps({"publicKey": auth["public_key"] if auth else None}),
        ]

        run_process = await asyncio.create_subprocess_exec(
            *run_args,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        run_result, run_error = await run_process.communicate(input=miden_code.encode())

        if run_process.returncode != 0:
            print(
                f"Collection: {collection_id}, Function: {function_name} with code:\n{collection_code}\nfailed to run:")
            print(f"Run error: {run_error.decode()}")
            print(f"run_args: {run_args}")
            return False

        this_json = None
        for line in run_result.decode().split("\n"):
            if line.startswith("this_json: "):
                this_json = line.split("this_json: ")[1]

        our_output_result = json.loads(this_json)

        # Filter out all None values from our_output_result, deeply. this_json omits None values, but output_result does not.
        def remove_none(x):
            if isinstance(x, dict):
                return {k: remove_none(v) for k, v in x.items() if v is not None}
            elif isinstance(x, list):
                return [remove_none(v) for v in x if v is not None]
            else:
                return x

        our_output_result = remove_none(our_output_result)

        if our_output_result != output_result["Ok"]["instance"]:
            print(
                f"Collection: {collection_id}, Function: {function_name} with code:\n{collection_code}\nfailed to match output:")
            print(f"Expected: {output_result}")
            print(f"Actual: {our_output_result}")
            return False

        print(
            f"Progress: {'{:.2f}'.format(round((i + 1) / total_lines * 100, 2))}%\tCompleted Tasks/Total = {i+1}/{total_lines}", file=sys.stderr)
        return True


async def run_tests():
    successes = 0
    failures = 0

    semaphore = asyncio.Semaphore(os.cpu_count())

    with open("function_test.jsonl", "r") as file:
        lines = file.readlines()
        lines = random.sample(lines, 10000)
        total_lines = len(lines)
        tasks = []
        for (i, line) in enumerate(lines):
            if i > 0:
                call_data = json.loads(line)
                collection_code = call_data["collection_code"]
                collection_id = call_data["collection_id"]
                collection_name = collection_id.split("/")[-1]
                function_name = call_data["function_name"]
                this_instance_json = call_data["instance"]
                this_instance = json.loads(this_instance_json)
                args = call_data["args"]
                auth_json = call_data["auth"]
                auth = json.loads(auth_json)
                output_result_json = call_data["output"]
                output_result = json.loads(output_result_json)

                if collection_id == "Collection":
                    collection_code = collection_code.replace(
                        "publicKey?: string", "publicKey?: PublicKey")
                    continue

                task = run_commands(semaphore, collection_code, collection_id, collection_name,
                                    function_name, this_instance_json, args, auth, output_result, i, total_lines)
                tasks.append(task)

        results = await asyncio.gather(*tasks)

        successes = results.count(True)
        failures = results.count(False)
        total = len(tasks)
        print(f"Successes: {successes}")
        print(f"Failures: {failures}")
        print(
            f"Total: {total}, Successes/Total = {round(successes / total * 100, 2)}%")


def main():
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command")

    parser_export = subparsers.add_parser("export")

    parser_test = subparsers.add_parser("test")

    args = parser.parse_args()

    if args.command == "export":
        export_from_bigquery()
    elif args.command == "test":
        asyncio.run(run_tests())


if __name__ == "__main__":
    main()
