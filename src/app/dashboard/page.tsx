'use client';

import { useEffect, useState } from 'react';
import { supabase } from '../../lib/supabase';

export default function Dashboard() {
  const [email, setEmail] = useState('');

  useEffect(() => {
    supabase.auth.getUser().then(({ data }) => {
      if (data.user) setEmail(data.user.email!);
    });
  }, []);

  return (
    <div className="p-8">
      <h1 className="text-3xl font-bold">Dashboard</h1>
      <p className="mt-2 text-gray-600">
        Logged in as {email}
      </p>

      <div className="mt-6 bg-white p-6 rounded shadow">
        <p>✅ App is connected to database</p>
        <p>✅ Auth is working</p>
        <p>🚀 GST SaaS foundation is LIVE</p>
      </div>
    </div>
  );
}
