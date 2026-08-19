```markdown
# Localhost Web Server Audit Cheat Sheet

## 1. Build and Launch Server
```bash
cargo build --release && ./target/release/localhost

```

---

## 2. CGI Execution Setup & Verification

Create the executable script:

```bash
mkdir -p www/cgi-bin
cat << 'EOF' > www/cgi-bin/test.py
#!/usr/bin/env python3
print("Content-Type: text/html\r\n\r\n<h1>CGI Working!</h1>")
EOF
chmod +x www/cgi-bin/test.py

```

Execute request:

```bash
curl -i [http://127.0.0.1:8080/cgi-bin/test.py](http://127.0.0.1:8080/cgi-bin/test.py)

```

* **Expected:** `HTTP/1.1 200 OK` and body `<h1>CGI Working!</h1>`.

---

## 3. Custom 404 Error Page

```bash
curl -i [http://127.0.0.1:8080/nonexistent_route_test](http://127.0.0.1:8080/nonexistent_route_test)

```

* **Expected:** `HTTP/1.1 404 Not Found` returning custom HTML error template.

---

## 4. Payload Limit Enforcement (413)

Send a 10MB payload:

```bash
curl -i -X POST -d "$(head -c 10000000 /dev/zero)" [http://127.0.0.1:8080/upload.txt](http://127.0.0.1:8080/upload.txt)

```

* **Expected:** `HTTP/1.1 413 Payload Too Large`.

---

## 5. Method Restriction Protection

```bash
curl -i -X DELETE [http://127.0.0.1:8080/](http://127.0.0.1:8080/)

```

* **Expected:** `HTTP/1.1 403 Forbidden` or `HTTP/1.1 405 Method Not Allowed`.

---

## 6. High-Concurrency Stress Benchmark

Install `siege` (WSL / Ubuntu):

```bash
sudo apt update && sudo apt install -y siege

```

Run test:

```bash
siege -b -t 10s [http://127.0.0.1:8080/](http://127.0.0.1:8080/)

```

* **Expected:** 100% Availability, 0 Failed Transactions.

```

```