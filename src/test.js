import http from 'k6/http';
import { sleep } from 'k6';

export const options = {
  stages: [
    { duration: '2s', target: 10000 },
  ],
};

export default function () {
  const BASE_URL = 'http://localhost:3000'; // make sure this is not production

  const responses = http.batch([
    ['GET', `${BASE_URL}/users/2`, null, { tags: { name: 'User query' } }],
  ]);

  sleep(1);
}

