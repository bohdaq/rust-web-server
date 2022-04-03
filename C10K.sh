#!/bin/bash
echo "Bash version ${BASH_VERSION}..."
for i in {0..10000}
  do 
     echo "GET / HTTP/1.1\r\n" | nc 127.0.0.1 7888 &
 done
