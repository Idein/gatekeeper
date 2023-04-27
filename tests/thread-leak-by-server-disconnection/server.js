// RUST_LOG=trace cargo run -- --rule example.yml --port=18080
// しておく
const http = require("http");
const cp = require("child_process");

async function a(){
        const srv = http.createServer((req, res) => {
                res.writeHead(200, { "Content-Type": "text/plain" });
                //res.end("Hello World"); // (リークしない)
                //res.socket.end(); // (リークする
                //res.socket.destroy(); // (リークする
                res.socket.resetAndDestroy(); // RST (リークする
        });
        srv.listen(3000);
        const curl = cp.exec("curl -v -x socks5h://0.0.0.0:18080 http://localhost:3000/");
        curl.stdout.pipe(process.stdout);
        curl.stderr.pipe(process.stderr);
        await new Promise((resolve, reject) => {
                curl.on("exit", resolve);
        });
        srv.close();
}

a().catch(console.error);
