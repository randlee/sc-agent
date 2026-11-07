"""Fake producer: writes diagnostic noise, then argv[1] as its last stderr line, and exits 2."""

import sys

sys.stderr.write("fake-producer: starting\n")
sys.stderr.write("fake-producer: something went wrong\n")
sys.stderr.write(sys.argv[1] + "\n\n")
sys.exit(2)
