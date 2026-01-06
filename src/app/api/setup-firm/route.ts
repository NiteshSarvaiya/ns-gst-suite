import { NextResponse } from 'next/server';
import { createClient } from '@supabase/supabase-js';

const supabase = createClient(
  process.env.NEXT_PUBLIC_SUPABASE_URL!,
  process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY!
);

export async function POST(req: Request) {
  const { email } = await req.json();

  const { data: firm } = await supabase
    .from('firm')
    .insert({ name: 'My CA Firm' })
    .select()
    .single();

  if (firm) {
    await supabase.from('app_user').insert({
      firm_id: firm.id,
      email,
      role: 'OWNER',
    });
  }

  return NextResponse.json({ success: true });
}
