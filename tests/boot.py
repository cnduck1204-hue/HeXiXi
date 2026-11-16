"""Boot in QEMU, run hello_world in the user shell, and wait for its return."""

import os
from pathlib import Path
import select
import subprocess
import time


def main():
    root = Path(__file__).resolve().parent.parent
    process = subprocess.Popen(
        ['bash', 'tools/run-qemu.sh'], cwd=root,
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )
    output = bytearray()
    command_sent = False
    deadline = time.monotonic() + 60
    try:
        while time.monotonic() < deadline:
            ready, _, _ = select.select([process.stdout], [], [], 1)
            if not ready:
                continue
            chunk = os.read(process.stdout.fileno(), 4096)
            if not chunk:
                raise RuntimeError(f'QEMU exited before test completion: {process.poll()}')
            output.extend(chunk)
            if not command_sent and b'>> ' in output:
                if '核夕夕 HeXiXi | based on rCore'.encode() not in output:
                    raise RuntimeError('Missing HeXiXi kernel banner')
                if b'HeXiXi user shell\n>> ' not in output:
                    raise RuntimeError('Missing HeXiXi user shell banner')
                process.stdin.write(b'hello_world\n')
                process.stdin.flush()
                command_sent = True
            if b'Hello world from user mode program!\n>> ' in output:
                print('PASS: HeXiXi banner, user shell, hello_world, return to shell')
                return
        raise TimeoutError('Boot test did not finish within 60 seconds')
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        print(output.decode('utf-8', errors='replace'))
        process.stdin.close()
        process.stdout.close()


if __name__ == '__main__':
    main()
