#!/usr/bin/env python3
import os

print("Content-Type: text/html\r\n\r\n")
print("<h1>Hello from Python CGI!</h1>")
print(f"<p><b>Request Method:</b> {os.environ.get('REQUEST_METHOD')}</p>")
print(f"<p><b>Query String:</b> {os.environ.get('QUERY_STRING')}</p>")