// Deterministic demo data for screenshots.
//   node tools/gen-sample.mjs samples/sales.csv 300      small file (in-memory mode)
//   node tools/gen-sample.mjs big.csv 2000000            large file (streaming mode, not committed)
import { createWriteStream } from 'node:fs';
const [out = 'samples/sales.csv', n = '300'] = process.argv.slice(2);
const N = +n;
let seed = 42;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const pick = (a) => a[Math.floor(rnd() * a.length)];
const first = ['Ava', 'Liam', 'Noah', 'Emma', 'Olivia', 'Mia', 'Lucas', 'Sofia', 'Ethan', 'Isla', 'Aarav', 'Priya', 'Mateo', 'Yuki', 'Chen', 'Fatima', 'Omar', 'Elena', 'Hugo', 'Zara'];
const last = ['Smith', 'Patel', 'Garcia', 'Kim', 'Nguyen', 'Müller', 'Rossi', 'Silva', 'Khan', 'Brown', 'Tanaka', 'Ivanov', 'Dubois', 'Lopez', 'Singh'];
const region = ['North America', 'Europe', 'Asia Pacific', 'Latin America', 'Middle East'];
const product = [['Standing Desk', 349], ['Ergo Chair', 289], ['Monitor Arm', 79], ['USB-C Dock', 129], ['Webcam 4K', 99], ['Mechanical Keyboard', 149], ['Desk Lamp', 59], ['Noise-Cancelling Headset', 199]];
const status = ['Delivered', 'Delivered', 'Delivered', 'Shipped', 'Processing', 'Returned'];
const q = (s) => (/[",\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s);
const w = createWriteStream(out);
w.write('Order ID,Order Date,Customer,Region,Product,Quantity,Unit Price,Total,Status,Notes\n');
let i = 0;
(function go() {
  let ok = true;
  while (i < N && ok) {
    const [p, price] = pick(product), qty = 1 + Math.floor(rnd() * 5);
    const d = new Date(Date.UTC(2025, 0, 1) + Math.floor(rnd() * 600) * 864e5).toISOString().slice(0, 10);
    const note = rnd() < 0.15 ? 'Gift wrap, "fragile" sticker' : rnd() < 0.1 ? 'Call before delivery' : '';
    ok = w.write([10000 + i, d, pick(first) + ' ' + pick(last), pick(region), p, qty, price.toFixed(2), (qty * price).toFixed(2), pick(status), q(note)].join(',') + '\n');
    i++;
  }
  if (i < N) w.once('drain', go); else w.end();
})();
