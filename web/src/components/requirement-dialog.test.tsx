import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { emptyRequirement } from '../lib/query'
import { RequirementDialog } from './RequirementDialog'

describe('RequirementDialog', () => {
  it('hides tier when a weapon item is selected', async () => {
    const user = userEvent.setup()
    render(<RequirementDialog open requirement={emptyRequirement('weapon')} onClose={vi.fn()} onSave={vi.fn()} />)
    expect(screen.getByLabelText('Tier')).toBeInTheDocument()
    await user.selectOptions(screen.getByLabelText('Item'), 'sword')
    expect(screen.queryByLabelText('Tier')).not.toBeInTheDocument()
  })
  it('removes curse options when uncursed is selected', async () => {
    const user = userEvent.setup()
    render(<RequirementDialog open requirement={emptyRequirement('weapon')} onClose={vi.fn()} onSave={vi.fn()} />)
    expect(screen.getByRole('option', { name: 'Annoying' })).toBeInTheDocument()
    await user.click(screen.getByLabelText(/Must be uncursed/))
    expect(screen.queryByRole('option', { name: 'Annoying' })).not.toBeInTheDocument()
  })
  it('caps ring upgrade input at four', async () => {
    const user = userEvent.setup()
    render(<RequirementDialog open requirement={emptyRequirement('weapon')} onClose={vi.fn()} onSave={vi.fn()} />)
    await user.click(screen.getByRole('button', { name: 'Ring' }))
    await user.selectOptions(screen.getByLabelText('Upgrade mode'), 'exact')
    expect(screen.getByLabelText('Upgrade level')).toHaveAttribute('max', '4')
  })
})
