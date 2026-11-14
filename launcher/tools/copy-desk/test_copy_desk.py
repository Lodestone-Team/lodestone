"""Tests for copy_desk.py's server: python3 -m unittest test_copy_desk (in launcher/tools/copy-desk)"""

import http.client
import json
import socketserver
import tempfile
import threading
import unittest
from http.server import ThreadingHTTPServer
from pathlib import Path

import copy_desk

class LocalServer(ThreadingHTTPServer):
    """HTTPServer without its reverse lookup of 127.0.0.1, which can take half a minute."""

    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


EDIT = {"id": "launcher/common/CommonMsg/Save", "texts": ["Keep"], "original": ["Save"]}


class Server(unittest.TestCase):
    """The desk on a free port, saving to a temporary edits.json."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.edits = copy_desk.EDITS
        copy_desk.EDITS = Path(self.tmp.name) / "edits.json"
        self.server = LocalServer(("127.0.0.1", 0), copy_desk.Handler)
        self.port = self.server.server_port
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        copy_desk.EDITS = self.edits
        self.tmp.cleanup()

    def request(self, method, path, body=None, headers=None):
        connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        try:
            # Host as given (a browser always sends one), not http.client's own
            connection.putrequest(method, path, skip_host=True, skip_accept_encoding=True)
            data = json.dumps(body).encode() if body is not None else b""
            for name, value in (headers or {}).items():
                connection.putheader(name, value)
            if method == "POST":
                connection.putheader("Content-Length", str(len(data)))
            connection.endheaders(data)
            response = connection.getresponse()
            return response.status, response.read()
        finally:
            connection.close()

    def page_headers(self, **extra):
        """What the desk's own page sends."""
        headers = {"Host": f"127.0.0.1:{self.port}", "Origin": f"http://127.0.0.1:{self.port}",
                   "Content-Type": "application/json"}
        headers.update(extra)
        return headers

    def test_the_desks_page_saves_and_reads_edits(self):
        status, _ = self.request("POST", "/api/edits", EDIT, self.page_headers())
        self.assertEqual(status, 200)
        status, body = self.request("GET", "/api/edits", headers={"Host": f"127.0.0.1:{self.port}"})
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)[EDIT["id"]]["texts"], ["Keep"])

    def test_localhost_works_too(self):
        headers = self.page_headers(Host=f"localhost:{self.port}",
                                    Origin=f"http://localhost:{self.port}")
        self.assertEqual(self.request("POST", "/api/edits", EDIT, headers)[0], 200)
        self.assertTrue(copy_desk.EDITS.exists())

    def test_another_pages_plain_text_post_is_refused(self):
        # what a page anywhere can send without asking first (no-cors, text/plain)
        headers = self.page_headers(Origin="https://example.com", **{"Content-Type": "text/plain"})
        self.assertEqual(self.request("POST", "/api/edits", EDIT, headers)[0], 403)
        self.assertFalse(copy_desk.EDITS.exists())

    def test_a_post_from_another_origin_is_refused_even_as_json(self):
        headers = self.page_headers(Origin="https://example.com")
        self.assertEqual(self.request("POST", "/api/edits", EDIT, headers)[0], 403)
        self.assertFalse(copy_desk.EDITS.exists())

    def test_a_post_without_an_origin_is_refused(self):
        headers = self.page_headers()
        del headers["Origin"]
        self.assertEqual(self.request("POST", "/api/edits", EDIT, headers)[0], 403)
        self.assertFalse(copy_desk.EDITS.exists())

    def test_a_post_that_isnt_json_is_refused(self):
        headers = self.page_headers(**{"Content-Type": "text/plain;charset=UTF-8"})
        self.assertEqual(self.request("POST", "/api/edits", EDIT, headers)[0], 403)
        self.assertFalse(copy_desk.EDITS.exists())

    def test_another_host_name_is_refused(self):
        # DNS rebinding: a page on evil.example whose name now resolves to 127.0.0.1
        host = f"evil.example:{self.port}"
        copy_desk.save_edits({EDIT["id"]: {"texts": ["Keep"], "original": ["Save"]}})
        self.assertEqual(self.request("GET", "/api/edits", headers={"Host": host})[0], 403)
        headers = self.page_headers(Host=host, Origin=f"http://{host}")
        self.assertEqual(self.request("POST", "/api/edits", dict(EDIT, texts=None), headers)[0], 403)
        self.assertIn(EDIT["id"], copy_desk.load_edits())


if __name__ == "__main__":
    unittest.main()
